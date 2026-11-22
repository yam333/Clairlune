//! Live wallpaper windows behind the desktop icons.
//!
//! The shell has two layouts, told apart at runtime:
//! - **Classic** (Windows 10, Windows 11 before 24H2): asked with message
//!   `0x052C`, Progman spawns a top-level `WorkerW` behind the one holding
//!   `SHELLDLL_DefView` (the icons). Our window becomes a child of it.
//! - **Raised desktop** (Windows 11 24H2 and later): Progman has
//!   `WS_EX_NOREDIRECTIONBITMAP` and holds the `SHELLDLL_DefView` and a
//!   `WorkerW` (which draws the wallpaper) as children. As Microsoft advises
//!   wallpaper apps, our window is a `WS_EX_LAYERED` child of Progman at full
//!   opacity, z-ordered under the DefView and above that `WorkerW`.
//!
//! The windows are plain Win32 windows, not winit ones: they take no input
//! and their messages are handled here. Whoever creates them must pump
//! messages on that thread (winit's loop does; see [`pump_messages_until`]),
//! because Explorer, their parent's owner, sends them messages and waits.

use crate::{Monitor, PlatformError};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, Win32WindowHandle, WindowHandle, WindowsDisplayHandle,
};
use std::num::NonZeroIsize;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use windows::Win32::Foundation::{COLORREF, HANDLE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{MapWindowPoints, ValidateRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{
    HPOWERNOTIFY, POWERBROADCAST_SETTING, RegisterPowerSettingNotification,
    UnregisterPowerSettingNotification,
};
use windows::Win32::System::SystemServices::GUID_CONSOLE_DISPLAY_STATE;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DEVICE_NOTIFY_WINDOW_HANDLE, DefWindowProcW, DestroyWindow, DispatchMessageW,
    EnumWindows, FindWindowExW, FindWindowW, GW_HWNDLAST, GWL_EXSTYLE, GWL_STYLE, GetParent,
    GetWindow, GetWindowLongPtrW, HWND_BOTTOM, IsWindow, LWA_ALPHA, MA_NOACTIVATE, MSG,
    MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PBT_POWERSETTINGCHANGE, PM_REMOVE,
    PeekMessageW, QS_ALLINPUT, RegisterClassExW, SMTO_NORMAL, SW_SHOWNA, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageTimeoutW, SetLayeredWindowAttributes,
    SetParent, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, WM_ERASEBKGND,
    WM_MOUSEACTIVATE, WM_PAINT, WM_POWERBROADCAST, WNDCLASSEXW, WS_CHILD, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{BOOL, w};

/// Set when the system asks any live window to repaint (it was uncovered or
/// moved), so the next frame is drawn whole.
static REPAINT: AtomicBool = AtomicBool::new(false);

/// The displays are off (power notification to the live windows).
static DISPLAYS_OFF: AtomicBool = AtomicBool::new(false);

/// Where live windows go: behind the icons, in one of the two layouts.
#[derive(Clone, Copy, Debug)]
pub struct DesktopLayer {
    progman: HWND,
    defview: HWND,
    workerw: HWND,
    raised: bool,
}

impl DesktopLayer {
    /// Finds the desktop layer, asking Progman to create it when needed.
    pub fn find() -> Result<DesktopLayer, PlatformError> {
        let progman = unsafe { FindWindowW(w!("Progman"), None) }
            .map_err(|e| PlatformError::Os(format!("no Progman window: {e}")))?;
        let ex = unsafe { GetWindowLongPtrW(progman, GWL_EXSTYLE) } as u32;
        let raised = ex & WS_EX_NOREDIRECTIONBITMAP.0 != 0;
        // Spawns the WorkerW behind the icons; nothing happens if it exists.
        unsafe {
            SendMessageTimeoutW(
                progman,
                0x052C,
                WPARAM(0xD),
                LPARAM(1),
                SMTO_NORMAL,
                1000,
                None,
            )
        };
        let missing = |what: &str| PlatformError::Os(format!("no {what} in the desktop layer"));
        if raised {
            let defview =
                unsafe { FindWindowExW(Some(progman), None, w!("SHELLDLL_DefView"), None) }
                    .map_err(|_| missing("SHELLDLL_DefView"))?;
            let workerw = unsafe { FindWindowExW(Some(progman), None, w!("WorkerW"), None) }
                .map_err(|_| missing("WorkerW"))?;
            return Ok(DesktopLayer {
                progman,
                defview,
                workerw,
                raised,
            });
        }
        // Classic: the top-level window holding the DefView, then the WorkerW
        // that comes after it.
        let mut found: (HWND, HWND) = (HWND::default(), HWND::default());
        unsafe extern "system" fn each(top: HWND, data: LPARAM) -> BOOL {
            let found = unsafe { &mut *(data.0 as *mut (HWND, HWND)) };
            if let Ok(defview) =
                unsafe { FindWindowExW(Some(top), None, w!("SHELLDLL_DefView"), None) }
                && let Ok(workerw) = unsafe { FindWindowExW(None, Some(top), w!("WorkerW"), None) }
            {
                *found = (defview, workerw);
            }
            BOOL(1)
        }
        let _ = unsafe { EnumWindows(Some(each), LPARAM(&mut found as *mut _ as isize)) };
        let (defview, workerw) = found;
        if workerw.is_invalid() {
            return Err(missing("WorkerW behind the icons"));
        }
        Ok(DesktopLayer {
            progman,
            defview,
            workerw,
            raised,
        })
    }

    /// "raised" (Windows 11 24H2+) or "classic".
    pub fn layout(&self) -> &'static str {
        if self.raised { "raised" } else { "classic" }
    }

    /// The window our windows are children of.
    fn parent(&self) -> HWND {
        if self.raised {
            self.progman
        } else {
            self.workerw
        }
    }

    /// False once Explorer has restarted or rebuilt the desktop: find it again.
    pub fn is_alive(&self) -> bool {
        let alive = |h: HWND| unsafe { IsWindow(Some(h)) }.as_bool();
        alive(self.progman) && alive(self.workerw) && (!self.raised || alive(self.defview))
    }

    /// Keeps the wallpaper `WorkerW` last among Progman's children (raised
    /// layout), so it never ends up over our windows.
    fn keep_workerw_at_bottom(&self) {
        if !self.raised {
            return;
        }
        let last = unsafe { GetWindow(self.defview, GW_HWNDLAST) }.unwrap_or_default();
        if last != self.workerw {
            let _ = unsafe {
                SetWindowPos(
                    self.workerw,
                    Some(HWND_BOTTOM),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                )
            };
        }
    }
}

/// One live wallpaper window covering a monitor, behind the icons.
#[derive(Debug)]
pub struct LiveWindow {
    hwnd: HWND,
    parent: HWND,
    width: u32,
    height: u32,
    /// Display on/off notifications come to this window.
    power: Option<HPOWERNOTIFY>,
}

const CLASS: windows::core::PCWSTR = w!("ClairluneLiveWallpaper");

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        // The frame covers everything; erasing first would flicker.
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let _ = unsafe { ValidateRect(Some(hwnd), None) };
            REPAINT.store(true, Ordering::Relaxed);
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_POWERBROADCAST if wp.0 as u32 == PBT_POWERSETTINGCHANGE && lp.0 != 0 => {
            let setting = unsafe { &*(lp.0 as *const POWERBROADCAST_SETTING) };
            if setting.PowerSetting == GUID_CONSOLE_DISPLAY_STATE {
                // 0 off, 1 on, 2 dimmed.
                let off = setting.Data[0] == 0;
                if DISPLAYS_OFF.swap(off, Ordering::Relaxed) != off {
                    tracing::info!(target: "clairlune", displays_off = off, "display state");
                }
            }
            LRESULT(1)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn register_class() -> Result<(), PlatformError> {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    let ok = *REGISTERED.get_or_init(|| {
        let Ok(module) = (unsafe { GetModuleHandleW(None) }) else {
            return false;
        };
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: module.into(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        let atom = unsafe { RegisterClassExW(&class) };
        atom != 0
    });
    if ok {
        Ok(())
    } else {
        Err(PlatformError::Os(
            "could not register the live window class".into(),
        ))
    }
}

impl LiveWindow {
    /// Creates a window over `monitor` in the desktop layer, hidden until
    /// [`LiveWindow::show`] (draw its first frame first).
    pub fn create(layer: &DesktopLayer, monitor: &Monitor) -> Result<LiveWindow, PlatformError> {
        register_class()?;
        let module =
            unsafe { GetModuleHandleW(None) }.map_err(|e| PlatformError::Os(e.to_string()))?;
        let mut ex = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        if layer.raised {
            ex |= WS_EX_LAYERED;
        }
        let (w, h) = (monitor.width as i32, monitor.height as i32);
        let hwnd = unsafe {
            CreateWindowExW(
                ex,
                CLASS,
                w!("Clairlune live wallpaper"),
                WS_POPUP,
                monitor.x,
                monitor.y,
                w,
                h,
                None,
                None,
                Some(module.into()),
                None,
            )
        }
        .map_err(|e| PlatformError::Os(format!("creating the live window: {e}")))?;
        let power = unsafe {
            RegisterPowerSettingNotification(
                HANDLE(hwnd.0),
                &GUID_CONSOLE_DISPLAY_STATE,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
        }
        .ok();
        let window = LiveWindow {
            hwnd,
            parent: layer.parent(),
            width: monitor.width,
            height: monitor.height,
            power,
        };
        if layer.raised {
            // Opaque layered window; set before SetParent (some renderers
            // lose WS_EX_LAYERED when it's added afterwards).
            unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA) }
                .map_err(|e| PlatformError::Os(format!("layered window: {e}")))?;
        }
        let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
        unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWL_STYLE,
                ((style & !WS_POPUP.0) | WS_CHILD.0) as isize,
            )
        };
        unsafe { SetParent(hwnd, Some(layer.parent())) }
            .map_err(|e| PlatformError::Os(format!("moving into the desktop layer: {e}")))?;
        // The monitor's corner in the parent's coordinates (the virtual
        // screen's origin can be negative).
        let mut corner = [POINT {
            x: monitor.x,
            y: monitor.y,
        }];
        unsafe { MapWindowPoints(None, Some(layer.parent()), &mut corner) };
        let (insert_after, order) = if layer.raised {
            (Some(layer.defview), SWP_NOACTIVATE) // just under the icons
        } else {
            (None, SWP_NOACTIVATE | SWP_NOZORDER)
        };
        unsafe { SetWindowPos(hwnd, insert_after, corner[0].x, corner[0].y, w, h, order) }
            .map_err(|e| PlatformError::Os(format!("placing the live window: {e}")))?;
        layer.keep_workerw_at_bottom();
        tracing::info!(
            target: "clairlune",
            layout = layer.layout(),
            monitor = %monitor.id,
            x = corner[0].x,
            y = corner[0].y,
            w,
            h,
            "live window in the desktop layer"
        );
        Ok(window)
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The window handle, for presenters that take one directly.
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Shows the window without activating anything.
    pub fn show(&self) {
        let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNA) };
    }

    /// Still a child of the desktop layer (Explorer restarts destroy it).
    pub fn is_attached(&self) -> bool {
        unsafe { IsWindow(Some(self.hwnd)) }.as_bool()
            && unsafe { GetParent(self.hwnd) }.is_ok_and(|p| p == self.parent)
    }
}

impl Drop for LiveWindow {
    fn drop(&mut self) {
        if let Some(p) = self.power.take() {
            let _ = unsafe { UnregisterPowerSettingNotification(p) };
        }
        if unsafe { IsWindow(Some(self.hwnd)) }.as_bool() {
            let _ = unsafe { DestroyWindow(self.hwnd) };
        }
    }
}

impl HasWindowHandle for LiveWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let hwnd = NonZeroIsize::new(self.hwnd.0 as isize).ok_or(HandleError::Unavailable)?;
        let raw = RawWindowHandle::Win32(Win32WindowHandle::new(hwnd));
        // The window lives as long as `self`.
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

/// The Windows display handle, for renderers that want one.
#[derive(Clone, Copy, Debug, Default)]
pub struct Win32Display;

impl HasDisplayHandle for Win32Display {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let raw = RawDisplayHandle::Windows(WindowsDisplayHandle::new());
        Ok(unsafe { DisplayHandle::borrow_raw(raw) })
    }
}

/// Whether a live window was asked to repaint since the last call.
pub fn take_repaint() -> bool {
    REPAINT.swap(false, Ordering::Relaxed)
}

/// The displays are off (known once a live window exists).
pub fn displays_off() -> bool {
    DISPLAYS_OFF.load(Ordering::Relaxed)
}

/// The session is locked, or another secure desktop (a UAC prompt) is up:
/// the wallpaper isn't visible. The input desktop can't be opened then.
pub fn session_locked() -> bool {
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_SWITCHDESKTOP, OpenInputDesktop,
    };
    match unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_SWITCHDESKTOP) } {
        Ok(desk) => {
            let _ = unsafe { CloseDesktop(desk) };
            false
        }
        Err(_) => true,
    }
}

/// Running on battery power.
pub fn on_battery() -> bool {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut s) }.is_ok() && s.ACLineStatus == 0
}

/// Handles this thread's window messages until `deadline`, sleeping between
/// them. For callers without an event loop (the CLI); the app's winit loop
/// does this itself.
pub fn pump_messages_until(deadline: Instant) {
    loop {
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let now = Instant::now();
        if now >= deadline {
            return;
        }
        let ms = (deadline - now).as_millis().min(u128::from(u32::MAX)) as u32;
        unsafe { MsgWaitForMultipleObjectsEx(None, ms, QS_ALLINPUT, MWMO_INPUTAVAILABLE) };
    }
}

/// CPU time used by this process so far (user + kernel), for budgets.
pub fn process_cpu_time() -> std::time::Duration {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let (mut c, mut e, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    if unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) }.is_err() {
        return std::time::Duration::ZERO;
    }
    let ticks = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
    // 100 ns ticks.
    std::time::Duration::from_nanos((ticks(k) + ticks(u)) * 100)
}

/// Hands memory the process isn't using back to the system (after a window
/// closes: what it touched stays resident otherwise).
pub fn trim_memory() {
    use windows::Win32::System::ProcessStatus::K32EmptyWorkingSet;
    use windows::Win32::System::Threading::GetCurrentProcess;
    let _ = unsafe { K32EmptyWorkingSet(GetCurrentProcess()) };
}

/// This process's working set (resident memory) in bytes.
pub fn process_memory() -> u64 {
    use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::GetCurrentProcess;
    let mut pmc = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc, pmc.cb) }.as_bool();
    if ok { pmc.WorkingSetSize as u64 } else { 0 }
}

/// For each monitor: is its work area hidden behind one window (a maximized
/// or full-screen app)? Live frames there are wasted, and DWM throttles a
/// covered window's presents (a 300 ms stall per frame here), so the live
/// wallpaper pauses on those monitors.
///
/// Only opaque, ordinary windows count: minimized, cloaked (other virtual
/// desktops), click-through and layered windows (possibly translucent
/// overlays) are ignored, which errs on the side of drawing.
pub fn covered(monitors: &[Monitor]) -> Vec<bool> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetWindowRect, IsIconic, IsWindowVisible, WS_EX_TRANSPARENT,
    };

    struct Scan {
        areas: Vec<RECT>,
        covered: Vec<bool>,
    }
    let areas = monitors
        .iter()
        .map(|m| {
            let full = RECT {
                left: m.x,
                top: m.y,
                right: m.x + m.width as i32,
                bottom: m.y + m.height as i32,
            };
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let hmon = unsafe { MonitorFromRect(&full, MONITOR_DEFAULTTONEAREST) };
            if unsafe { GetMonitorInfoW(hmon, &mut info) }.as_bool() {
                info.rcWork
            } else {
                full
            }
        })
        .collect();
    let mut scan = Scan {
        covered: vec![false; monitors.len()],
        areas,
    };
    unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> BOOL {
        let scan = unsafe { &mut *(data.0 as *mut Scan) };
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() || unsafe { IsIconic(hwnd) }.as_bool() {
            return BOOL(1);
        }
        let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
        // (WS_EX_NOREDIRECTIONBITMAP only means DirectComposition: Chromium
        // and WinUI apps have it and are opaque, so they count.)
        if ex & (WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0) != 0 {
            return BOOL(1);
        }
        let mut cloaked = 0u32;
        let _ = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED,
                (&mut cloaked as *mut u32).cast(),
                std::mem::size_of::<u32>() as u32,
            )
        };
        if cloaked != 0 {
            return BOOL(1);
        }
        let mut class = [0u16; 32];
        let n = unsafe { GetClassNameW(hwnd, &mut class) } as usize;
        let class = String::from_utf16_lossy(&class[..n.min(class.len())]);
        if class == "Progman" || class == "WorkerW" {
            return BOOL(1);
        }
        let mut r = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut r) }.is_err() {
            return BOOL(1);
        }
        for (i, a) in scan.areas.iter().enumerate() {
            if r.left <= a.left && r.top <= a.top && r.right >= a.right && r.bottom >= a.bottom {
                scan.covered[i] = true;
            }
        }
        BOOL(1)
    }
    let _ = unsafe { EnumWindows(Some(each), LPARAM(&mut scan as *mut Scan as isize)) };
    scan.covered
}

/// The graphics adapter a monitor is plugged into, as PCI vendor and device
/// ids plus its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayAdapter {
    pub vendor: u32,
    pub device: u32,
    pub name: String,
}

/// For each monitor, the adapter that drives it (DXGI outputs). Rendering on
/// another adapter makes the system copy every frame across, which stalls
/// (seen here: 300 ms every second rendering on the iGPU for a monitor on the
/// discrete GPU), so live wallpapers render on this one.
pub fn display_adapters(monitors: &[Monitor]) -> Vec<Option<DisplayAdapter>> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
    let mut out = vec![None; monitors.len()];
    let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else {
        return out;
    };
    for a in 0.. {
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(a) }) else {
            break;
        };
        let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
            continue;
        };
        let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(128);
        let info = DisplayAdapter {
            vendor: desc.VendorId,
            device: desc.DeviceId,
            name: String::from_utf16_lossy(&desc.Description[..len]),
        };
        for o in 0.. {
            let Ok(output) = (unsafe { adapter.EnumOutputs(o) }) else {
                break;
            };
            let Ok(od) = (unsafe { output.GetDesc() }) else {
                continue;
            };
            let r = od.DesktopCoordinates;
            for (i, m) in monitors.iter().enumerate() {
                if r.left == m.x
                    && r.top == m.y
                    && r.right - r.left == m.width as i32
                    && r.bottom - r.top == m.height as i32
                {
                    out[i] = Some(info.clone());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pause-rule probes answer without failing (their values depend on
    /// the session: a CI service session can count as locked).
    #[test]
    fn pause_probes_answer() {
        let (locked, battery, off) = (session_locked(), on_battery(), displays_off());
        eprintln!("locked {locked}, on battery {battery}, displays off {off}");
        assert!(!off, "nothing has reported the displays off in a test");
    }
}

/// Set by the watcher window: the monitor layout changed (plugged, moved,
/// resolution, DPI or work area).
static DISPLAYS_CHANGED: AtomicBool = AtomicBool::new(false);
/// Set by the watcher window: Explorer started again (its taskbar was made).
static TASKBAR_CREATED: AtomicBool = AtomicBool::new(false);
/// Set by the watcher window: the clock or time zone changed, or the
/// computer woke up.
static CLOCK_CHANGED: AtomicBool = AtomicBool::new(false);
/// The id of the "TaskbarCreated" message, once registered.
static TASKBAR_MSG: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

unsafe extern "system" fn watcher_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_SETWORKAREA, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_SETTINGCHANGE,
    };
    let taskbar = TASKBAR_MSG.load(Ordering::Relaxed);
    if taskbar != 0 && msg == taskbar {
        TASKBAR_CREATED.store(true, Ordering::Relaxed);
        return LRESULT(0);
    }
    // Resume from sleep (automatic, and by the user).
    const PBT_APMRESUMESUSPEND: u32 = 0x7;
    const PBT_APMRESUMEAUTOMATIC: u32 = 0x12;
    match msg {
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            DISPLAYS_CHANGED.store(true, Ordering::Relaxed);
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_TIMECHANGE => {
            CLOCK_CHANGED.store(true, Ordering::Relaxed);
            LRESULT(0)
        }
        WM_POWERBROADCAST
            if matches!(wp.0 as u32, PBT_APMRESUMESUSPEND | PBT_APMRESUMEAUTOMATIC) =>
        {
            CLOCK_CHANGED.store(true, Ordering::Relaxed);
            LRESULT(1)
        }
        WM_SETTINGCHANGE if wp.0 as u32 == SPI_SETWORKAREA.0 => {
            DISPLAYS_CHANGED.store(true, Ordering::Relaxed);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

/// A hidden top-level window that hears the system's broadcasts: display
/// changes and Explorer restarts (message-only windows don't get them). Its
/// messages also wake the thread's event loop, so nothing needs polling.
#[derive(Debug)]
pub struct SystemWatcher {
    hwnd: HWND,
}

impl SystemWatcher {
    pub fn new() -> Result<SystemWatcher, PlatformError> {
        use windows::Win32::UI::WindowsAndMessaging::RegisterWindowMessageW;
        const WATCHER: windows::core::PCWSTR = w!("ClairluneSystemWatcher");
        static REGISTERED: OnceLock<bool> = OnceLock::new();
        let module =
            unsafe { GetModuleHandleW(None) }.map_err(|e| PlatformError::Os(e.to_string()))?;
        let ok = *REGISTERED.get_or_init(|| {
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(watcher_proc),
                hInstance: module.into(),
                lpszClassName: WATCHER,
                ..Default::default()
            };
            let atom = unsafe { RegisterClassExW(&class) };
            atom != 0
        });
        if !ok {
            return Err(PlatformError::Os(
                "could not register the watcher class".into(),
            ));
        }
        TASKBAR_MSG.store(
            unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
            Ordering::Relaxed,
        );
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                WATCHER,
                w!("Clairlune system watcher"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(module.into()),
                None,
            )
        }
        .map_err(|e| PlatformError::Os(format!("creating the watcher window: {e}")))?;
        Ok(SystemWatcher { hwnd })
    }

    /// Whether the monitor layout changed since the last call.
    pub fn take_displays_changed(&self) -> bool {
        DISPLAYS_CHANGED.swap(false, Ordering::Relaxed)
    }

    /// Whether Explorer restarted since the last call.
    pub fn take_taskbar_created(&self) -> bool {
        TASKBAR_CREATED.swap(false, Ordering::Relaxed)
    }

    /// Whether the clock or time zone changed, or the computer woke, since
    /// the last call.
    pub fn take_clock_changed(&self) -> bool {
        CLOCK_CHANGED.swap(false, Ordering::Relaxed)
    }
}

impl Drop for SystemWatcher {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

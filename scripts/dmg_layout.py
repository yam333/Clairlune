"""Where things are in the DMG's window, in points: the icons Finder shows
(scripts/make_dmg.py) and the background drawn around them
(scripts/dmg_background.py)."""

W, H = 720, 564
ICON = 96
TEXT = 13
# The background goes on below the window's edge: title bars differ in height.
BLEED = 12

# Icon centres.
APP = (222, 168)
APPLICATIONS = (498, 168)
SECURITY = (360, 432)

# The alias to System Settings > Privacy & Security, and what it opens. (An
# alias to the pane opens from a downloaded DMG; a .webloc or .inetloc with
# the page's x-apple.systempreferences address gets a warning of its own, or
# doesn't open.)
SECURITY_NAME = "Privacy & Security"
SECURITY_PANE = "/System/Library/PreferencePanes/Security.prefPane"

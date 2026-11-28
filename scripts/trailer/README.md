# The trailer

The trailer (`clairlune-trailer.mp4`, attached to a release; not kept in
the repository) is made from the app itself: scenes rendered by
`clairlune-cli`, the Settings window from the app's UI test, and an original
soundtrack synthesised by `music.py` (no samples). To make it again:

```sh
cargo build --release -p clairlune-cli
# Settings screenshots: the macOS artifact of the "Check: Settings
# screenshots" workflow (check-ui.yml), unzipped into <ui dir>
python scripts/trailer/music.py <work>/music.wav
python scripts/trailer/make_trailer.py --ffmpeg <ffmpeg> --ui <ui dir> \
    --work <work> --music <work>/music.wav --out docs/media/trailer.mp4
```

It also writes `trailer.jpg` (a poster frame, not kept) and
`trailer-preview.webp`, a
silent animated preview of the first five shots that the README shows: GitHub
plays animated images in a README, but not a video kept in the repository.

Both scripts need numpy; `make_trailer.py` also needs Pillow and an ffmpeg
with libx264 (the one `pip install imageio-ffmpeg` brings works). The shots,
captions and cuts are listed at the top of `make_trailer.py`; the cuts fall on
the music's bar lines. Captions say what Clairlune does, never how many
landscapes or activities it has (those numbers grow), and the Settings panels
are cropped below the page header for the same reason.

The video is the `clairlune-trailer.mp4` asset of the v0.28.0 release, so its
address never changes: upload a new one there with
`gh release upload v0.28.0 clairlune-trailer.mp4 --clobber`, and copy
`trailer-preview.webp` to `site/assets/` too (the website's poster).

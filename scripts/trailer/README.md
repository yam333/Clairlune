# The trailer

`docs/media/trailer.mp4` is made from the app itself: scenes rendered by
`clairlune-cli`, the Settings window from the app's UI test, and an original
soundtrack synthesised by `music.py` (no samples). To make it again:

```sh
cargo build --release -p clairlune -p clairlune-cli
target/release/clairlune --ui-test <ui dir>          # Settings screenshots
python scripts/trailer/music.py <work>/music.wav
python scripts/trailer/make_trailer.py --ffmpeg <ffmpeg> --ui <ui dir> \
    --work <work> --music <work>/music.wav --out docs/media/trailer.mp4
```

It also writes `trailer.jpg` (a poster frame) and `trailer-preview.webp`, a
silent animated preview of the first five shots that the README shows: GitHub
plays animated images in a README, but not a video kept in the repository.

Both scripts need numpy; `make_trailer.py` also needs Pillow and an ffmpeg
with libx264 (the one `pip install imageio-ffmpeg` brings works). The shots,
captions and cuts are listed at the top of `make_trailer.py`; the cuts fall on
the music's bar lines.

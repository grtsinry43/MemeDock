#!/usr/bin/env bash
# Synthetic fixtures only; print hexadecimal bytes for review/apply_patch.
# Usage: bash tools/native-media/generate-creator-fixtures.sh h264|hevc|rotated|vfr|still
set -euo pipefail
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
case "${1:?h264 or hevc required}" in
  h264) ffmpeg -hide_banner -loglevel error -f lavfi -i 'testsrc2=size=64x48:rate=10:duration=3' \
    -c:v libx264 -preset ultrafast -crf 28 -pix_fmt yuv420p -an "$work/sample.mp4" ;;
  hevc) ffmpeg -hide_banner -loglevel error -f lavfi -i 'testsrc2=size=64x48:rate=10:duration=2' \
    -c:v libx265 -x265-params 'pools=1:frame-threads=1:log-level=error' -pix_fmt yuv420p \
    -tag:v hvc1 -an "$work/sample.mp4" ;;
  rotated)
    ffmpeg -hide_banner -loglevel error -f lavfi -i 'testsrc2=size=64x48:rate=10:duration=3' \
      -c:v libx264 -preset ultrafast -crf 28 -pix_fmt yuv420p -an "$work/original.mp4"
    ffmpeg -hide_banner -loglevel error -display_rotation:v:0 90 -i "$work/original.mp4" -c copy "$work/sample.mp4" ;;
  vfr) ffmpeg -hide_banner -loglevel error -f lavfi -i 'testsrc2=size=64x48:rate=10:duration=2' \
    -vf 'setpts=PTS+if(gte(N\,10)\,0.5/TB\,0)' -fps_mode vfr -c:v libx264 \
    -preset ultrafast -crf 28 -pix_fmt yuv420p -an "$work/sample.mp4" ;;
  still) ffmpeg -hide_banner -loglevel error -f lavfi -i 'color=c=red:size=64x48:rate=10:duration=4' \
    -c:v libx264 -preset ultrafast -pix_fmt yuv420p -an "$work/sample.mp4" ;;
  *) exit 2 ;;
esac
xxd -p "$work/sample.mp4"

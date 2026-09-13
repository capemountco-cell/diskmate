# Aperture — a more visual DiskMate

Six artboards for a redraw of the interface, answering one note: too much
text, make it look like a Mac app.

The rule the direction follows is that a number or a shape says it, and a
sentence only appears where a number on its own would mislead. The capacity
meter and the category legend are the same object. The inspector's paragraph
about what DiskMate can account for is a bar and a two-word pill. A cleanup
finding's explanation is a small bar chart of the biggest things inside it.
The duplicate pipeline is four steps you can read at a glance instead of four
disconnected tiles.

| Artboard | Screen |
| --- | --- |
| `Main.dc.html` | Storage, with the treemap and the inspector |
| `Scanning.dc.html` | A scan in progress: throughput, cells filling in, nothing estimated |
| `Cleanup.dc.html` | Findings as cards, the staged tray, and free space after |
| `Duplicates.dc.html` | How 2 million files became 39 sets, and what was opened to prove it |
| `Memory.dc.html` | Sixty seconds of pressure, and what is holding it |
| `Dark.dc.html` | Storage again, dark |

Every colour comes from `src/styles/tokens.css`. The category hues are the
solved, colour-vision-checked set and must not be nudged by hand here either.

## Rebuilding

The artboards are assembled from `parts/`, because the sidebar and the theme
blocks are shared and would otherwise drift across six files.

```sh
cd design/aperture
node parts/mkscreens.mjs          # regenerates the four generated screens
./build.sh                        # reassembles all six .dc.html artboards
```

Then re-seed the canvas with the `design` skill's helper, passing every
artboard plus `canvas.json`, and republish to the same artifact.

To see an artboard in a browser without the canvas editor, swap the
`support.js` line for `<style>x-dc{display:block}helmet{display:none}</style>`
and open the file.

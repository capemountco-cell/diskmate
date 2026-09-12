# Interface designs

Mockups of the DiskMate interface, authored as Design Components and published
as one canvas. They are a proposal, not a spec: the app is the source of truth
for anything already built.

| File                 | Screen                                                  |
| -------------------- | ------------------------------------------------------- |
| `Main.dc.html`       | Storage: treemap, category strip, and the inspector      |
| `Cleanup.dc.html`    | Cleanup: findings on the left, the open queue on the right |
| `Duplicates.dc.html` | Duplicates: a set shown as a comparison, with what was read |
| `Memory.dc.html`     | Memory: app memory stacked on cache, and the position on RAM |
| `canvas.json`        | Where each artboard sits, and the notes explaining why   |

Every value comes from `src/styles/tokens.css` — the 232px sidebar, the 12px
panel radius, the 13px base on the system face, the hairline rules and the
seven category hues. Nothing here invents a colour. Each artboard carries a
light/dark switch driven by the same `data-appearance` attribute the real
application uses, so a change to the tokens can be checked against both.

The numbers are plausible sample data shaped like a developer's Mac. They are
not readings from any real machine, and no screen here is wired to the engine.

## Rebuilding the canvas

The published page bundles an editor around these files, so it is build output
rather than source and is not committed. Regenerate it from the sources with
the `/design` skill's seeding helper, then publish `diskmate-interface.html`.

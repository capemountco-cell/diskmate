import { Note } from "../../components/primitives";

/**
 * Cleanup is not implemented.
 *
 * This page says so, and says what will be true when it is. It deliberately
 * shows no counts, no "potential savings" figure and no button: a number here
 * would be a promise the product cannot yet keep, and inventing one is exactly
 * the behaviour DiskMate exists to avoid.
 */
export function CleanupPage() {
  return (
    <div className="placeholder-page">
      <p className="muted">
        Cleanup is not active yet. DiskMate currently reads your disk and nothing else: there is
        no code in this build that deletes, moves or changes a file.
      </p>

      <Note tone="info" title="How this will work.">
        Every candidate will arrive with a reason and a risk level, and nothing will be removed
        without you choosing it. Items that can be regenerated are treated differently from items
        that cannot, and anything DiskMate protects is never offered at all.
      </Note>

      <div>
        <h3 className="section-heading">Planned</h3>
        <ul className="placeholder-page__list">
          <li>Large and old files, with the context to judge them</li>
          <li>Downloads and the Trash</li>
          <li>Duplicates, confirmed by content rather than by name</li>
          <li>Developer build artefacts and caches</li>
          <li>Application leftovers</li>
        </ul>
      </div>

      <p className="faint">
        Removal will move items to the Trash rather than deleting them, and every action will go
        through one place in the code so that the safety checks cannot be bypassed by a feature.
      </p>
    </div>
  );
}

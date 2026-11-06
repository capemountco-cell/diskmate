import { useEffect, useState, type ReactNode } from "react";

import { AgeMap } from "../../components/AgeMap";
import { Panel, Segmented, Spinner } from "../../components/primitives";
import { Sunburst } from "../../components/Sunburst";
import { Treemap } from "../../components/Treemap";
import { describeError, engine } from "../../lib/ipc";
import type { AgeBucket, EntryRow } from "../../lib/types";

type View = "map" | "rings" | "age" | "browse";

/**
 * Four ways to read the same scan.
 *
 * Not eight. Each of these answers a question the others answer badly: the map
 * compares sizes, the rings show depth and containment, the age view finds what
 * is big *and* stale, and the list is what you use when you already know where
 * you are going. Adding a fifth rendering of "proportions" would be a longer
 * feature list and a worse product.
 */
export function StorageViews({
  scanId,
  root,
  onOpen,
  explorer,
  selectedId = null,
  onSelect,
}: {
  scanId: number;
  root: EntryRow;
  onOpen: (row: EntryRow) => void;
  explorer: ReactNode;
  selectedId?: number | null;
  onSelect?: (row: EntryRow) => void;
}) {
  const [view, setView] = useState<View>("map");
  const [rows, setRows] = useState<EntryRow[]>([]);
  const [ages, setAges] = useState<AgeBucket[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Loaded once per scan and shared by the map and the rings: it is the same
  // bounded slice of the tree, drawn two ways.
  useEffect(() => {
    if (view !== "map" && view !== "rings") return;
    if (rows.length > 0) return;
    let cancelled = false;
    setLoading(true);
    engine
      .subtree(scanId, root.id, 4, 24)
      .then((result) => !cancelled && setRows(result))
      .catch((cause) => !cancelled && setError(describeError(cause)))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [view, scanId, root.id, rows.length]);

  useEffect(() => {
    if (view !== "age" || ages.length > 0) return;
    let cancelled = false;
    setLoading(true);
    engine
      .ageBreakdown(scanId)
      .then((result) => !cancelled && setAges(result))
      .catch((cause) => !cancelled && setError(describeError(cause)))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [view, scanId, ages.length]);

  const note = {
    map: "",
    rings: "Each ring is one folder deeper.",
    age: "What is big is rarely the question. What is big and untouched usually is.",
    browse: "Double-click a folder to look inside.",
  }[view];

  return (
    <Panel
      title="Where the space is"
      note={note}
      action={
        <Segmented<View>
          label="How to view this scan"
          value={view}
          onChange={setView}
          options={[
            { value: "map", label: "Map" },
            { value: "rings", label: "Rings" },
            { value: "age", label: "Age" },
            { value: "browse", label: "Browse" },
          ]}
        />
      }
      flush={view === "browse"}
    >
      {error && <p className="muted">{error}</p>}
      {loading && <Spinner />}

      {view === "map" && !loading && (
        <Treemap
          rows={rows}
          rootId={root.id}
          onOpen={onOpen}
          selectedId={selectedId}
          onSelect={onSelect}
        />
      )}
      {view === "rings" && !loading && (
        <Sunburst rows={rows} rootId={root.id} onOpen={onOpen} />
      )}
      {view === "age" && !loading && <AgeMap buckets={ages} />}
      {view === "browse" && explorer}
    </Panel>
  );
}

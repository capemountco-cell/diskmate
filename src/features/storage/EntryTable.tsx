import { FileIcon, FolderIcon, LinkIcon, LockIcon } from "../../components/icons";
import { formatBytes, formatCount, formatDate, formatPercent } from "../../lib/format";
import type { EntryRow, SortDirection, SortKey } from "../../lib/types";

/**
 * One page of directory contents.
 *
 * The table renders what it is given and nothing more. Sorting, paging and
 * filtering are all queries against the index, so a folder with a million
 * children costs exactly as much to display as one with ten.
 */
export function EntryTable({
  rows,
  total,
  sort,
  direction,
  onSort,
  onOpen,
  onSelect,
  emptyMessage,
}: {
  rows: EntryRow[];
  total: number;
  sort: SortKey;
  direction: SortDirection;
  onSort: (key: SortKey) => void;
  onOpen: (row: EntryRow) => void;
  /** A single click inspects; a double click still opens. */
  onSelect?: (row: EntryRow) => void;
  emptyMessage: string;
}) {
  if (rows.length === 0) {
    return (
      <p className="muted" style={{ padding: "var(--space-5)" }}>
        {emptyMessage}
      </p>
    );
  }

  return (
    <>
      <table className="table">
        <thead>
          <tr>
            <SortableHeader
              label="Name"
              column="name"
              sort={sort}
              direction={direction}
              onSort={onSort}
            />
            <SortableHeader
              label="Size"
              column="size"
              sort={sort}
              direction={direction}
              onSort={onSort}
              numeric
            />
            <th className="table__numeric">Share</th>
            <SortableHeader
              label="Items"
              column="items"
              sort={sort}
              direction={direction}
              onSort={onSort}
              numeric
            />
            <SortableHeader
              label="Modified"
              column="modified"
              sort={sort}
              direction={direction}
              onSort={onSort}
              numeric
            />
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <EntryTableRow key={row.id} row={row} onOpen={onOpen} onSelect={onSelect} />
          ))}
        </tbody>
      </table>

      {total > rows.length && (
        <p className="faint" style={{ padding: "var(--space-3) var(--space-4)" }}>
          Showing the {formatCount(rows.length)} largest of {formatCount(total)} items.
        </p>
      )}
    </>
  );
}

function EntryTableRow({
  row,
  onOpen,
  onSelect,
}: {
  row: EntryRow;
  onOpen: (row: EntryRow) => void;
  onSelect?: (row: EntryRow) => void;
}) {
  const navigable = row.kind === "directory" && row.accessible;

  return (
    <tr
      data-navigable={navigable}
      onClick={onSelect ? () => onSelect(row) : undefined}
      onDoubleClick={navigable ? () => onOpen(row) : undefined}
      // A row that can be opened is a control, so it behaves like one for the
      // keyboard as well as the mouse.
      tabIndex={navigable ? 0 : -1}
      onKeyDown={
        navigable
          ? (event) => {
              if (event.key === "Enter") onOpen(row);
            }
          : undefined
      }
    >
      <td className="table__cell--truncate">
        <div className="table__name">
          <EntryIcon row={row} />
          <span className="table__name-text" title={row.path}>
            {row.name}
          </span>
          {!row.accessible && row.kind === "directory" && (
            <span className="badge" title="DiskMate could not read inside this folder">
              <LockIcon size={11} /> Not readable
            </span>
          )}
        </div>
      </td>
      <td className="table__numeric">
        {formatBytes(row.size)}
        {row.sharesStorage && (
          <div className="faint" style={{ fontSize: "var(--text-xs)" }}>
            shared copy
          </div>
        )}
      </td>
      <td className="share-cell">
        <span
          className="share-cell__bar"
          style={{ width: `${Math.min(row.shareOfParent * 100, 100)}%` }}
        />
        <span className="share-cell__text">{formatPercent(row.shareOfParent)}</span>
      </td>
      <td className="table__numeric">
        {row.kind === "directory" ? formatCount(row.fileCount + row.dirCount) : "—"}
      </td>
      <td className="table__numeric faint">{formatDate(row.modifiedAt)}</td>
    </tr>
  );
}

function EntryIcon({ row }: { row: EntryRow }) {
  if (row.kind === "directory") return <FolderIcon className="entry-icon entry-icon--folder" />;
  if (row.kind === "symlink") return <LinkIcon className="entry-icon" />;
  return <FileIcon className="entry-icon" />;
}

function SortableHeader({
  label,
  column,
  sort,
  direction,
  onSort,
  numeric,
}: {
  label: string;
  column: SortKey;
  sort: SortKey;
  direction: SortDirection;
  onSort: (key: SortKey) => void;
  numeric?: boolean;
}) {
  const active = sort === column;
  return (
    <th
      className={numeric ? "table__numeric" : undefined}
      aria-sort={active ? (direction === "asc" ? "ascending" : "descending") : undefined}
    >
      <button type="button" onClick={() => onSort(column)}>
        {label}
        {active && <span aria-hidden="true">{direction === "asc" ? "↑" : "↓"}</span>}
      </button>
    </th>
  );
}

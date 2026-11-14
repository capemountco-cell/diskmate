import { useEffect, useRef, useState } from "react";

import { Button } from "../../components/primitives";
import { WIPE_NEEDS_PERMISSION, describeError, engine, onWipe } from "../../lib/ipc";

const LENGTHS = [
  { seconds: 30, label: "30 sec" },
  { seconds: 60, label: "1 min" },
  { seconds: 120, label: "2 min" },
];

/**
 * Starting wipe-down mode: how long, what it does, and, the first time, the
 * Accessibility permission macOS needs before any app may pause the keyboard.
 */
export function WipeSheet({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [seconds, setSeconds] = useState(60);
  const [needsPermission, setNeedsPermission] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    if (!open) return;
    setError(null);
    const onKey = (event: KeyboardEvent) => event.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const start = async () => {
    setStarting(true);
    setError(null);
    try {
      await engine.wipeStart(seconds);
      setNeedsPermission(false);
      onClose();
    } catch (cause) {
      const message = describeError(cause);
      if (message === WIPE_NEEDS_PERMISSION) setNeedsPermission(true);
      else setError(message);
    } finally {
      setStarting(false);
    }
  };

  return (
    <div className="sheet" role="dialog" aria-modal="true" aria-labelledby="wipe-title">
      <button type="button" className="sheet__scrim" aria-label="Close" onClick={onClose} />
      <div className="sheet__panel">
        {needsPermission ? (
          <>
            <h2 id="wipe-title" className="sheet__title">
              Allow Mr Clean to pause the keyboard and trackpad
            </h2>
            <p className="sheet__body">
              macOS only lets an app ignore key presses and clicks if you allow it under
              Accessibility. Mr Clean uses this for wipe-down mode and nothing else, and only
              while the timer runs.
            </p>
            <ol className="sheet__steps">
              <li>Click Open System Settings, then turn on Mr Clean in the list.</li>
              <li>Come back here and click Start.</li>
            </ol>
            {error && <p className="sheet__error">{error}</p>}
            <div className="sheet__actions">
              <Button onClick={() => setNeedsPermission(false)}>Back</Button>
              <Button onClick={() => void engine.wipeRequestPermission()}>
                Open System Settings
              </Button>
              <Button variant="primary" onClick={() => void start()} disabled={starting}>
                Start
              </Button>
            </div>
          </>
        ) : (
          <>
            <h2 id="wipe-title" className="sheet__title">
              Wipe down this Mac
            </h2>
            <p className="sheet__body">
              The keyboard and trackpad ignore everything and the screen goes black, so you can
              clean them without typing or clicking anything. It unlocks by itself when the time
              is up. To stop early, hold Esc and Return together for 3 seconds.
            </p>
            <div className="seg" role="group" aria-label="How long">
              {LENGTHS.map((length) => (
                <button
                  key={length.seconds}
                  type="button"
                  aria-pressed={seconds === length.seconds}
                  onClick={() => setSeconds(length.seconds)}
                >
                  {length.label}
                </button>
              ))}
            </div>
            {error && <p className="sheet__error">{error}</p>}
            <div className="sheet__actions">
              <Button onClick={onClose}>Cancel</Button>
              <Button variant="primary" onClick={() => void start()} disabled={starting}>
                {starting ? "Starting…" : "Start"}
              </Button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

/**
 * What the screen shows while the keyboard and trackpad are locked: black,
 * with a ring that empties as the time runs out, and a second ring that fills
 * while Esc and Return are held. Then a short "done".
 */
export function WipeOverlay({ onAgain }: { onAgain: () => void }) {
  const [phase, setPhase] = useState<"off" | "locked" | "done">("off");
  const [total, setTotal] = useState(60);
  const [left, setLeft] = useState(60);
  const [held, setHeld] = useState(0);
  const endsAt = useRef(0);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void onWipe({
      started: (seconds) => {
        setTotal(seconds);
        setLeft(seconds);
        setHeld(0);
        endsAt.current = Date.now() + seconds * 1000;
        setPhase("locked");
      },
      held: setHeld,
      ended: () => {
        setHeld(0);
        setPhase("done");
      },
    }).then((fn) => (unlisten = fn));
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    if (phase !== "locked") return;
    const timer = window.setInterval(
      () => setLeft(Math.max(0, Math.ceil((endsAt.current - Date.now()) / 1000))),
      250,
    );
    return () => window.clearInterval(timer);
  }, [phase]);

  if (phase === "off") return null;

  if (phase === "done") {
    return (
      <div className="wipe wipe--done" role="status">
        <span className="status-chip status-chip--good">
          <i aria-hidden="true" />
          Unlocked
        </span>
        <p className="wipe__big">All done.</p>
        <p className="wipe__note">The keyboard and trackpad work again.</p>
        <div className="row">
          <Button
            onClick={() => {
              setPhase("off");
              onAgain();
            }}
          >
            Another round
          </Button>
          <Button variant="primary" onClick={() => setPhase("off")}>
            Close
          </Button>
        </div>
      </div>
    );
  }

  const circumference = 2 * Math.PI * 100;
  const minutes = Math.floor(left / 60);
  const seconds = String(left % 60).padStart(2, "0");
  return (
    <div
      className="wipe"
      role="timer"
      aria-label={`${minutes} minutes ${left % 60} seconds left`}
    >
      <div className="wipe__ring">
        <svg viewBox="0 0 220 220" aria-hidden="true">
          <circle cx="110" cy="110" r="100" className="wipe__track" />
          <circle
            cx="110"
            cy="110"
            r="100"
            className="wipe__left"
            strokeDasharray={circumference}
            strokeDashoffset={circumference * (1 - left / Math.max(total, 1))}
            transform="rotate(-90 110 110)"
          />
        </svg>
        <b className="num">
          {minutes}:{seconds}
        </b>
      </div>
      <p className="wipe__title">Keyboard and trackpad are locked. Wipe away.</p>
      <p className="wipe__note">
        The screen is black so smudges are easy to see. Everything unlocks by itself when the
        ring empties.
      </p>
      <p className="wipe__hint">
        To stop early, hold <kbd>esc</kbd> + <kbd>return</kbd> for 3 seconds
        <span className="wipe__hold" aria-hidden="true">
          <i style={{ width: `${held * 100}%` }} />
        </span>
      </p>
    </div>
  );
}

import React, { useState } from "react";
import Modal from "@/components/ui/Modal";
import ErrorBanner from "@/components/ui/ErrorBanner";
import { resultsService } from "@/services/results.service";
import { useToast } from "@/context/ToastContext";
import type { EventResult, MotorsportEvent, ResultUnit } from "@/types";
import styles from "./ResultFormModal.module.css";

// ── Shared option arrays ──────────────────────────────────────────────────────

const UNITS: Array<{ value: ResultUnit; label: string }> = [
  { value: "milliseconds", label: "Milliseconds" },
  { value: "seconds", label: "Seconds" },
  { value: "meters", label: "Meters" },
  { value: "kilometers", label: "Kilometers" },
  { value: "feet", label: "Feet" },
  { value: "inches", label: "Inches" },
  { value: "kilograms", label: "Kilograms" },
  { value: "points", label: "Points" },
];

// ── Record-result form ────────────────────────────────────────────────────────

interface RecordFormProps {
  events: MotorsportEvent[];
  onSuccess: (result: EventResult) => void;
  onClose: () => void;
}

function RecordResultForm({ events, onSuccess, onClose }: RecordFormProps) {
  const { addToast } = useToast();
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [eventId, setEventId] = useState<string>(
    events.length > 0 ? String(events[0].id) : ""
  );
  const [participantId, setParticipantId] = useState("");
  const [attemptNo, setAttemptNo] = useState("1");
  const [valueNumeric, setValueNumeric] = useState("");
  const [unit, setUnit] = useState<ResultUnit>("points");

  const isValid =
    eventId !== "" &&
    participantId.trim() !== "" &&
    attemptNo.trim() !== "" &&
    valueNumeric.trim() !== "";

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!isValid) return;
    setError(null);
    setSubmitting(true);

    try {
      const result = await resultsService.createResult(Number(eventId), {
        participant_id: parseInt(participantId, 10),
        attempt_no: parseInt(attemptNo, 10),
        value_numeric: parseFloat(valueNumeric),
        unit_enum: unit,
      });
      addToast(
        `Result recorded for participant #${result.participant_id}.`,
        "success"
      );
      onSuccess(result);
      onClose();
    } catch (err: unknown) {
      setError(
        err instanceof Error ? err.message : "Failed to record result."
      );
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} noValidate aria-label="Record result form">
      {error && (
        <div className={styles.errorWrap}>
          <ErrorBanner message={error} />
        </div>
      )}

      <div className={styles.field}>
        <label htmlFor="res_event" className={styles.label}>
          Event <span className={styles.req}>*</span>
        </label>
        <select
          id="res_event"
          className={styles.input}
          value={eventId}
          onChange={(e) => setEventId(e.target.value)}
          disabled={submitting || events.length === 0}
        >
          {events.length === 0 ? (
            <option value="">No events available</option>
          ) : (
            events.map((ev) => (
              <option key={ev.id} value={String(ev.id)}>
                {ev.name}
              </option>
            ))
          )}
        </select>
      </div>

      <div className={styles.row}>
        <div className={styles.field}>
          <label htmlFor="participant_id" className={styles.label}>
            Participant ID <span className={styles.req}>*</span>
          </label>
          <input
            id="participant_id"
            className={styles.input}
            type="number"
            placeholder="1"
            min="1"
            value={participantId}
            onChange={(e) => setParticipantId(e.target.value)}
            disabled={submitting}
            required
          />
        </div>
        <div className={styles.field}>
          <label htmlFor="attempt_no" className={styles.label}>
            Attempt No. <span className={styles.req}>*</span>
          </label>
          <input
            id="attempt_no"
            className={styles.input}
            type="number"
            placeholder="1"
            min="1"
            value={attemptNo}
            onChange={(e) => setAttemptNo(e.target.value)}
            disabled={submitting}
            required
          />
        </div>
      </div>

      <div className={styles.row}>
        <div className={styles.field}>
          <label htmlFor="value_numeric" className={styles.label}>
            Value <span className={styles.req}>*</span>
          </label>
          <input
            id="value_numeric"
            className={styles.input}
            type="number"
            placeholder="98750"
            step="any"
            value={valueNumeric}
            onChange={(e) => setValueNumeric(e.target.value)}
            disabled={submitting}
            required
          />
        </div>
        <div className={styles.field}>
          <label htmlFor="unit_enum" className={styles.label}>
            Unit <span className={styles.req}>*</span>
          </label>
          <select
            id="unit_enum"
            className={styles.input}
            value={unit}
            onChange={(e) => setUnit(e.target.value as ResultUnit)}
            disabled={submitting}
          >
            {UNITS.map((u) => (
              <option key={u.value} value={u.value}>
                {u.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      <div className={styles.actions}>
        <button
          type="button"
          className={styles.cancelBtn}
          onClick={onClose}
          disabled={submitting}
        >
          Cancel
        </button>
        <button
          type="submit"
          className={styles.submitBtn}
          disabled={submitting || !isValid}
          aria-busy={submitting}
        >
          {submitting ? "Recording…" : "Record Result"}
        </button>
      </div>
    </form>
  );
}

// ── Arbitrate form ────────────────────────────────────────────────────────────

interface ArbitrateFormProps {
  eventId: number;
  resultId: number;
  onSuccess: () => void;
  onClose: () => void;
}

function ArbitrateForm({
  eventId,
  resultId,
  onSuccess,
  onClose,
}: ArbitrateFormProps) {
  const { addToast } = useToast();
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [decision, setDecision] = useState<"approved" | "rejected">(
    "approved"
  );
  const [reason, setReason] = useState("");

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setSubmitting(true);

    try {
      await resultsService.arbitrate(eventId, resultId, {
        decision,
        reason: reason.trim() || undefined,
      });
      addToast(
        `Result #${resultId} arbitrated: ${decision}.`,
        decision === "approved" ? "success" : "info"
      );
      onSuccess();
      onClose();
    } catch (err: unknown) {
      setError(
        err instanceof Error ? err.message : "Arbitration failed."
      );
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} noValidate aria-label="Arbitrate result form">
      {error && (
        <div className={styles.errorWrap}>
          <ErrorBanner message={error} />
        </div>
      )}

      <p className={styles.contextNote}>
        Event ID: <strong>{eventId}</strong> · Result ID:{" "}
        <strong>{resultId}</strong>
      </p>

      <div className={styles.field}>
        <span className={styles.label}>
          Decision <span className={styles.req}>*</span>
        </span>
        <div className={styles.radioGroup}>
          {(["approved", "rejected"] as const).map((d) => (
            <label key={d} className={styles.radioLabel}>
              <input
                type="radio"
                name="decision"
                value={d}
                checked={decision === d}
                onChange={() => setDecision(d)}
                disabled={submitting}
              />
              {d === "approved" ? "Approve" : "Reject"}
            </label>
          ))}
        </div>
      </div>

      <div className={styles.field}>
        <label htmlFor="arb_reason" className={styles.label}>
          Reason
        </label>
        <textarea
          id="arb_reason"
          className={styles.textarea}
          rows={3}
          placeholder="Optional arbitration rationale…"
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          disabled={submitting}
        />
      </div>

      <div className={styles.actions}>
        <button
          type="button"
          className={styles.cancelBtn}
          onClick={onClose}
          disabled={submitting}
        >
          Cancel
        </button>
        <button
          type="submit"
          className={`${styles.submitBtn} ${
            decision === "rejected" ? styles.rejectBtn : ""
          }`}
          disabled={submitting}
          aria-busy={submitting}
        >
          {submitting
            ? "Submitting…"
            : decision === "approved"
            ? "Approve Result"
            : "Reject Result"}
        </button>
      </div>
    </form>
  );
}

// ── Public component ──────────────────────────────────────────────────────────

export type ResultFormMode = "record" | "arbitrate";

export interface ResultFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  mode: ResultFormMode;
  /** Required when mode === "record" */
  events?: MotorsportEvent[];
  /** Required when mode === "arbitrate" */
  eventId?: number;
  resultId?: number;
  onSuccess: (result?: EventResult) => void;
}

export default function ResultFormModal({
  isOpen,
  onClose,
  mode,
  events = [],
  eventId,
  resultId,
  onSuccess,
}: ResultFormModalProps) {
  const title =
    mode === "record" ? "Record New Result" : "Arbitrate Result";

  return (
    <Modal isOpen={isOpen} onClose={onClose} title={title} size="md">
      {mode === "record" ? (
        <RecordResultForm
          events={events}
          onSuccess={(r) => onSuccess(r)}
          onClose={onClose}
        />
      ) : eventId !== undefined && resultId !== undefined ? (
        <ArbitrateForm
          eventId={eventId}
          resultId={resultId}
          onSuccess={() => onSuccess()}
          onClose={onClose}
        />
      ) : null}
    </Modal>
  );
}

import React, { useState } from "react";
import Modal from "@/components/ui/Modal";
import ErrorBanner from "@/components/ui/ErrorBanner";
import { assetsService } from "@/services/assets.service";
import { useToast } from "@/context/ToastContext";
import type {
  Asset,
  AssetCategory,
  AssetStatus,
  CreateAssetPayload,
} from "@/types";
import styles from "./AssetFormModal.module.css";

// ── Shared option arrays ──────────────────────────────────────────────────────

const CATEGORIES: Array<{ value: AssetCategory; label: string }> = [
  { value: "vehicle", label: "Vehicle" },
  { value: "equipment", label: "Equipment" },
  { value: "facility", label: "Facility" },
  { value: "electronic", label: "Electronic" },
  { value: "other", label: "Other" },
];

const STATUSES: Array<{ value: AssetStatus; label: string }> = [
  { value: "in_service", label: "In Service" },
  { value: "out_for_repair", label: "Out for Repair" },
  { value: "retired", label: "Retired" },
];

// ── Create form ───────────────────────────────────────────────────────────────

interface CreateFormProps {
  onSuccess: (asset: Asset) => void;
  onClose: () => void;
}

function CreateAssetForm({ onSuccess, onClose }: CreateFormProps) {
  const { addToast } = useToast();
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [assetCode, setAssetCode] = useState("");
  const [category, setCategory] = useState<AssetCategory>("equipment");
  const [brand, setBrand] = useState("");
  const [model, setModel] = useState("");
  const [notes, setNotes] = useState("");
  const [cost, setCost] = useState("");
  const [date, setDate] = useState("");
  const [lifeMonths, setLifeMonths] = useState("");

  const isValid =
    assetCode.trim() !== "" && brand.trim() !== "" && model.trim() !== "";

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!isValid) return;
    setError(null);
    setSubmitting(true);

    const payload: CreateAssetPayload = {
      asset_code: assetCode.trim(),
      category,
      brand: brand.trim(),
      model: model.trim(),
    };
    if (notes.trim()) payload.notes = notes.trim();
    if (cost.trim()) payload.procurement_cost = cost.trim();
    if (date) payload.procurement_date = date;
    if (lifeMonths.trim())
      payload.useful_life_months = parseInt(lifeMonths, 10);

    try {
      const created = await assetsService.create(payload);
      addToast(`Asset ${created.asset_code} registered.`, "success");
      onSuccess(created);
      onClose();
    } catch (err: unknown) {
      setError(
        err instanceof Error ? err.message : "Failed to register asset."
      );
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form
      onSubmit={handleSubmit}
      noValidate
      aria-label="Create asset form"
    >
      {error && (
        <div className={styles.errorWrap}>
          <ErrorBanner message={error} />
        </div>
      )}

      <div className={styles.row}>
        <div className={styles.field}>
          <label htmlFor="asset_code" className={styles.label}>
            Asset Code <span className={styles.req}>*</span>
          </label>
          <input
            id="asset_code"
            className={styles.input}
            type="text"
            placeholder="ASSET-2024-001"
            value={assetCode}
            onChange={(e) => setAssetCode(e.target.value)}
            disabled={submitting}
            required
          />
        </div>
        <div className={styles.field}>
          <label htmlFor="category" className={styles.label}>
            Category <span className={styles.req}>*</span>
          </label>
          <select
            id="category"
            className={styles.input}
            value={category}
            onChange={(e) => setCategory(e.target.value as AssetCategory)}
            disabled={submitting}
          >
            {CATEGORIES.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      <div className={styles.row}>
        <div className={styles.field}>
          <label htmlFor="brand" className={styles.label}>
            Brand <span className={styles.req}>*</span>
          </label>
          <input
            id="brand"
            className={styles.input}
            type="text"
            placeholder="Toyota"
            value={brand}
            onChange={(e) => setBrand(e.target.value)}
            disabled={submitting}
            required
          />
        </div>
        <div className={styles.field}>
          <label htmlFor="model" className={styles.label}>
            Model <span className={styles.req}>*</span>
          </label>
          <input
            id="model"
            className={styles.input}
            type="text"
            placeholder="GR Yaris"
            value={model}
            onChange={(e) => setModel(e.target.value)}
            disabled={submitting}
            required
          />
        </div>
      </div>

      <div className={styles.row}>
        <div className={styles.field}>
          <label htmlFor="cost" className={styles.label}>
            Procurement Cost (AUD)
          </label>
          <input
            id="cost"
            className={styles.input}
            type="number"
            placeholder="45000.00"
            min="0"
            step="0.01"
            value={cost}
            onChange={(e) => setCost(e.target.value)}
            disabled={submitting}
          />
        </div>
        <div className={styles.field}>
          <label htmlFor="proc_date" className={styles.label}>
            Procurement Date
          </label>
          <input
            id="proc_date"
            className={styles.input}
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
            disabled={submitting}
          />
        </div>
      </div>

      <div className={styles.field}>
        <label htmlFor="life_months" className={styles.label}>
          Useful Life (months)
        </label>
        <input
          id="life_months"
          className={styles.input}
          type="number"
          placeholder="60"
          min="1"
          value={lifeMonths}
          onChange={(e) => setLifeMonths(e.target.value)}
          disabled={submitting}
        />
      </div>

      <div className={styles.field}>
        <label htmlFor="notes" className={styles.label}>
          Notes
        </label>
        <textarea
          id="notes"
          className={styles.textarea}
          rows={2}
          placeholder="Optional notes…"
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
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
          className={styles.submitBtn}
          disabled={submitting || !isValid}
          aria-busy={submitting}
        >
          {submitting ? "Registering…" : "Register Asset"}
        </button>
      </div>
    </form>
  );
}

// ── Status-update form ────────────────────────────────────────────────────────

interface StatusFormProps {
  asset: Asset;
  onSuccess: (asset: Asset) => void;
  onClose: () => void;
}

function UpdateStatusForm({ asset, onSuccess, onClose }: StatusFormProps) {
  const { addToast } = useToast();
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<AssetStatus>(asset.status);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (status === asset.status) {
      onClose();
      return;
    }
    setError(null);
    setSubmitting(true);

    try {
      const updated = await assetsService.updateStatus(asset.id, { status });
      addToast(`${asset.asset_code} status updated to "${status}".`, "success");
      onSuccess(updated);
      onClose();
    } catch (err: unknown) {
      setError(
        err instanceof Error ? err.message : "Failed to update status."
      );
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form
      onSubmit={handleSubmit}
      noValidate
      aria-label="Update asset status form"
    >
      {error && (
        <div className={styles.errorWrap}>
          <ErrorBanner message={error} />
        </div>
      )}

      <p className={styles.contextNote}>
        Asset: <strong>{asset.asset_code}</strong> — {asset.brand} {asset.model}
      </p>

      <div className={styles.field}>
        <label htmlFor="new_status" className={styles.label}>
          New Status <span className={styles.req}>*</span>
        </label>
        <select
          id="new_status"
          className={styles.input}
          value={status}
          onChange={(e) => setStatus(e.target.value as AssetStatus)}
          disabled={submitting}
        >
          {STATUSES.map((s) => (
            <option key={s.value} value={s.value}>
              {s.label}
            </option>
          ))}
        </select>
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
          disabled={submitting}
          aria-busy={submitting}
        >
          {submitting ? "Saving…" : "Save Status"}
        </button>
      </div>
    </form>
  );
}

// ── Public component ──────────────────────────────────────────────────────────

export type AssetFormMode = "create" | "update-status";

export interface AssetFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  mode: AssetFormMode;
  /** Required when mode === "update-status" */
  asset?: Asset;
  onSuccess: (asset: Asset) => void;
}

export default function AssetFormModal({
  isOpen,
  onClose,
  mode,
  asset,
  onSuccess,
}: AssetFormModalProps) {
  const title =
    mode === "create" ? "Register New Asset" : "Update Asset Status";

  return (
    <Modal isOpen={isOpen} onClose={onClose} title={title} size="md">
      {mode === "create" ? (
        <CreateAssetForm onSuccess={onSuccess} onClose={onClose} />
      ) : asset ? (
        <UpdateStatusForm asset={asset} onSuccess={onSuccess} onClose={onClose} />
      ) : null}
    </Modal>
  );
}

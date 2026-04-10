import React, { useState } from "react";
import Modal from "@/components/ui/Modal";
import ErrorBanner from "@/components/ui/ErrorBanner";
import { invoicesService } from "@/services/invoices.service";
import { useToast } from "@/context/ToastContext";
import type { Invoice } from "@/types";
import styles from "./InvoiceFormModal.module.css";

interface InvoiceFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreated: (invoice: Invoice) => void;
}

export default function InvoiceFormModal({
  isOpen,
  onClose,
  onCreated,
}: InvoiceFormModalProps) {
  const { addToast } = useToast();
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [counterparty, setCounterparty] = useState("");
  const [issueDate, setIssueDate] = useState(
    new Date().toISOString().slice(0, 10)   // default today
  );
  const [taxRate, setTaxRate] = useState("0.10");

  const isValid = counterparty.trim() !== "" && issueDate !== "";

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!isValid) return;
    setError(null);
    setSubmitting(true);

    try {
      const created = await invoicesService.create({
        counterparty: counterparty.trim(),
        issue_date: issueDate,
        tax_rate: taxRate.trim() || undefined,
      });
      addToast(`Invoice ${created.invoice_no} created.`, "success");
      onCreated(created);
      onClose();
    } catch (err: unknown) {
      setError(
        err instanceof Error ? err.message : "Failed to create invoice."
      );
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Create Invoice"
      size="sm"
    >
      <form onSubmit={handleSubmit} noValidate aria-label="Create invoice form">
        {error && (
          <div className={styles.errorWrap}>
            <ErrorBanner message={error} />
          </div>
        )}

        <div className={styles.field}>
          <label htmlFor="counterparty" className={styles.label}>
            Counterparty <span className={styles.req}>*</span>
          </label>
          <input
            id="counterparty"
            className={styles.input}
            type="text"
            placeholder="Acme Racing Pty Ltd"
            value={counterparty}
            onChange={(e) => setCounterparty(e.target.value)}
            disabled={submitting}
            required
          />
        </div>

        <div className={styles.field}>
          <label htmlFor="issue_date" className={styles.label}>
            Issue Date <span className={styles.req}>*</span>
          </label>
          <input
            id="issue_date"
            className={styles.input}
            type="date"
            value={issueDate}
            onChange={(e) => setIssueDate(e.target.value)}
            disabled={submitting}
            required
          />
        </div>

        <div className={styles.field}>
          <label htmlFor="tax_rate" className={styles.label}>
            Tax Rate{" "}
            <span className={styles.hint}>(decimal, e.g. 0.10 = 10%)</span>
          </label>
          <input
            id="tax_rate"
            className={styles.input}
            type="number"
            placeholder="0.10"
            step="0.01"
            min="0"
            max="1"
            value={taxRate}
            onChange={(e) => setTaxRate(e.target.value)}
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
            {submitting ? "Creating…" : "Create Invoice"}
          </button>
        </div>
      </form>
    </Modal>
  );
}

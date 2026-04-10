import React from "react";
import type { Invoice } from "@/types";
import { formatCurrency, formatDate } from "@/utils/format";
import styles from "./InvoicesTable.module.css";

interface InvoicesTableProps {
  invoices: Invoice[];
  page: number;
  totalPages: number;
  onPageChange: (p: number) => void;
  /** When provided, an "Issue" action button is shown on draft invoices */
  onIssue?: (invoice: Invoice) => void;
}

const STATUS_BADGE: Record<string, string> = {
  draft: styles.badgeGray,
  issued: styles.badgeBlue,
  paid: styles.badgeGreen,
  cancelled: styles.badgeRed,
  overdue: styles.badgeAmber,
};

function statusLabel(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

export default function InvoicesTable({
  invoices,
  page,
  totalPages,
  onPageChange,
  onIssue,
}: InvoicesTableProps) {
  return (
    <div className={styles.wrapper}>
      <div className={styles.tableScroll}>
        <table className={styles.table}>
          <thead>
            <tr>
              <th>Invoice No.</th>
              <th>Counterparty</th>
              <th>Issue Date</th>
              <th>Status</th>
              <th>Subtotal</th>
              <th>Total (inc. tax)</th>
              {onIssue && <th>Actions</th>}
            </tr>
          </thead>
          <tbody>
            {invoices.map((inv) => (
              <tr key={inv.id}>
                <td className={styles.invNo}>{inv.invoice_no}</td>
                <td>{inv.counterparty}</td>
                <td>{formatDate(inv.issue_date + "T00:00:00Z")}</td>
                <td>
                  <span
                    className={`${styles.badge} ${
                      STATUS_BADGE[inv.status] ?? ""
                    }`}
                    data-testid={`inv-status-${inv.id}`}
                  >
                    {statusLabel(inv.status)}
                  </span>
                </td>
                <td>{formatCurrency(inv.subtotal)}</td>
                <td className={styles.total}>{formatCurrency(inv.total)}</td>
                {onIssue && (
                  <td>
                    {inv.status === "draft" && (
                      <button
                        type="button"
                        className={styles.issueBtn}
                        onClick={() => onIssue(inv)}
                        aria-label={`Issue invoice ${inv.invoice_no}`}
                      >
                        Issue
                      </button>
                    )}
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {totalPages > 1 && (
        <div className={styles.pagination} aria-label="Pagination">
          <button
            type="button"
            onClick={() => onPageChange(page - 1)}
            disabled={page <= 1}
            className={styles.pageBtn}
            aria-label="Previous page"
          >
            ← Prev
          </button>
          <span className={styles.pageInfo}>
            Page {page} of {totalPages}
          </span>
          <button
            type="button"
            onClick={() => onPageChange(page + 1)}
            disabled={page >= totalPages}
            className={styles.pageBtn}
            aria-label="Next page"
          >
            Next →
          </button>
        </div>
      )}
    </div>
  );
}

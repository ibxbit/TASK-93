import React from "react";
import type { InvoiceStatus } from "@/types";
import styles from "./InvoicesSearch.module.css";

interface InvoicesSearchProps {
  search: string;
  onSearch: (v: string) => void;
  status: InvoiceStatus | "";
  onStatus: (v: InvoiceStatus | "") => void;
}

const STATUSES: Array<{ value: InvoiceStatus | ""; label: string }> = [
  { value: "", label: "All statuses" },
  { value: "draft", label: "Draft" },
  { value: "issued", label: "Issued" },
  { value: "paid", label: "Paid" },
  { value: "overdue", label: "Overdue" },
  { value: "cancelled", label: "Cancelled" },
];

export default function InvoicesSearch({
  search,
  onSearch,
  status,
  onStatus,
}: InvoicesSearchProps) {
  return (
    <div className={styles.bar}>
      <input
        className={styles.searchInput}
        type="search"
        placeholder="Search by counterparty or invoice no…"
        value={search}
        onChange={(e) => onSearch(e.target.value)}
        aria-label="Search invoices"
      />
      <select
        className={styles.select}
        value={status}
        onChange={(e) => onStatus(e.target.value as InvoiceStatus | "")}
        aria-label="Filter by status"
      >
        {STATUSES.map((s) => (
          <option key={s.value} value={s.value}>
            {s.label}
          </option>
        ))}
      </select>
    </div>
  );
}

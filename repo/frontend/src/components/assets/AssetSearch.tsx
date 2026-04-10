import React from "react";
import type { AssetCategory, AssetStatus } from "@/types";
import styles from "./AssetSearch.module.css";

interface AssetSearchProps {
  search: string;
  onSearch: (v: string) => void;
  category: AssetCategory | "";
  onCategory: (v: AssetCategory | "") => void;
  status: AssetStatus | "";
  onStatus: (v: AssetStatus | "") => void;
}

const CATEGORIES: Array<{ value: AssetCategory | ""; label: string }> = [
  { value: "", label: "All categories" },
  { value: "vehicle", label: "Vehicle" },
  { value: "equipment", label: "Equipment" },
  { value: "facility", label: "Facility" },
  { value: "electronic", label: "Electronic" },
  { value: "other", label: "Other" },
];

const STATUSES: Array<{ value: AssetStatus | ""; label: string }> = [
  { value: "", label: "All statuses" },
  { value: "in_service", label: "In Service" },
  { value: "out_for_repair", label: "Out for Repair" },
  { value: "retired", label: "Retired" },
];

export default function AssetSearch({
  search,
  onSearch,
  category,
  onCategory,
  status,
  onStatus,
}: AssetSearchProps) {
  return (
    <div className={styles.bar}>
      <input
        className={styles.searchInput}
        type="search"
        placeholder="Search by code, brand or model…"
        value={search}
        onChange={(e) => onSearch(e.target.value)}
        aria-label="Search assets"
      />
      <select
        className={styles.select}
        value={category}
        onChange={(e) => onCategory(e.target.value as AssetCategory | "")}
        aria-label="Filter by category"
      >
        {CATEGORIES.map((c) => (
          <option key={c.value} value={c.value}>
            {c.label}
          </option>
        ))}
      </select>
      <select
        className={styles.select}
        value={status}
        onChange={(e) => onStatus(e.target.value as AssetStatus | "")}
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

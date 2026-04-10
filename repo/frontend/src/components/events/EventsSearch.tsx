import React from "react";
import type { EventStatus } from "@/types";
import styles from "./EventsSearch.module.css";

interface EventsSearchProps {
  search: string;
  onSearch: (v: string) => void;
  status: EventStatus | "";
  onStatus: (v: EventStatus | "") => void;
}

const STATUSES: Array<{ value: EventStatus | ""; label: string }> = [
  { value: "", label: "All statuses" },
  { value: "draft", label: "Draft" },
  { value: "published", label: "Published" },
  { value: "in_progress", label: "In Progress" },
  { value: "completed", label: "Completed" },
  { value: "cancelled", label: "Cancelled" },
];

export default function EventsSearch({
  search,
  onSearch,
  status,
  onStatus,
}: EventsSearchProps) {
  return (
    <div className={styles.bar}>
      <input
        className={styles.searchInput}
        type="search"
        placeholder="Search by name, venue or group…"
        value={search}
        onChange={(e) => onSearch(e.target.value)}
        aria-label="Search events"
      />
      <select
        className={styles.select}
        value={status}
        onChange={(e) => onStatus(e.target.value as EventStatus | "")}
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

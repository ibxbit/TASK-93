import React from "react";
import type { MotorsportEvent } from "@/types";
import { formatDate, labelEventStatus } from "@/utils/format";
import styles from "./EventsTable.module.css";

interface EventsTableProps {
  events: MotorsportEvent[];
  page: number;
  totalPages: number;
  onPageChange: (p: number) => void;
}

const STATUS_BADGE: Record<string, string> = {
  draft: styles.badgeGray,
  published: styles.badgeBlue,
  in_progress: styles.badgeAmber,
  completed: styles.badgeGreen,
  cancelled: styles.badgeRed,
};

export default function EventsTable({
  events,
  page,
  totalPages,
  onPageChange,
}: EventsTableProps) {
  return (
    <div className={styles.wrapper}>
      <div className={styles.tableScroll}>
        <table className={styles.table}>
          <thead>
            <tr>
              <th>Name</th>
              <th>Venue</th>
              <th>Group</th>
              <th>Status</th>
              <th>Championship</th>
              <th>Created</th>
            </tr>
          </thead>
          <tbody>
            {events.map((e) => (
              <tr key={e.id}>
                <td className={styles.name}>{e.name}</td>
                <td>{e.venue_identifier ?? "—"}</td>
                <td>{e.schedule_group ?? "—"}</td>
                <td>
                  <span
                    className={`${styles.badge} ${STATUS_BADGE[e.status] ?? ""}`}
                    data-testid={`status-badge-${e.id}`}
                  >
                    {labelEventStatus(e.status)}
                  </span>
                </td>
                <td className={styles.championship}>
                  {e.is_championship_class ? (
                    <span className={styles.yes} aria-label="Yes">✓</span>
                  ) : (
                    <span className={styles.no} aria-label="No">—</span>
                  )}
                </td>
                <td>{formatDate(e.created_at)}</td>
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

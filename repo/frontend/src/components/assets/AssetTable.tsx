import React from "react";
import type { Asset } from "@/types";
import {
  formatCurrency,
  formatDate,
  labelAssetCategory,
  labelAssetStatus,
} from "@/utils/format";
import styles from "./AssetTable.module.css";

interface AssetTableProps {
  assets: Asset[];
  page: number;
  totalPages: number;
  onPageChange: (p: number) => void;
  /** When provided, an "Update Status" button is shown for each row. */
  onUpdateStatus?: (asset: Asset) => void;
}

const STATUS_BADGE: Record<string, string> = {
  in_service: styles.badgeGreen,
  out_for_repair: styles.badgeAmber,
  retired: styles.badgeGray,
};

export default function AssetTable({
  assets,
  page,
  totalPages,
  onPageChange,
  onUpdateStatus,
}: AssetTableProps) {
  return (
    <div className={styles.wrapper}>
      <div className={styles.tableScroll}>
        <table className={styles.table}>
          <thead>
            <tr>
              <th>Asset Code</th>
              <th>Category</th>
              <th>Brand / Model</th>
              <th>Status</th>
              <th>Procurement Cost</th>
              <th>Date</th>
              {onUpdateStatus && <th>Actions</th>}
            </tr>
          </thead>
          <tbody>
            {assets.map((a) => (
              <tr key={a.id}>
                <td className={styles.code}>{a.asset_code}</td>
                <td>{labelAssetCategory(a.category)}</td>
                <td>
                  <span className={styles.brand}>{a.brand}</span>{" "}
                  <span className={styles.model}>{a.model}</span>
                </td>
                <td>
                  <span
                    className={`${styles.badge} ${STATUS_BADGE[a.status] ?? ""}`}
                  >
                    {labelAssetStatus(a.status)}
                  </span>
                </td>
                <td>{formatCurrency(a.procurement_cost)}</td>
                <td>
                  {a.procurement_date
                    ? formatDate(a.procurement_date + "T00:00:00Z")
                    : "—"}
                </td>
                {onUpdateStatus && (
                  <td>
                    <button
                      type="button"
                      className={styles.actionBtn}
                      onClick={() => onUpdateStatus(a)}
                      aria-label={`Update status of ${a.asset_code}`}
                    >
                      Update Status
                    </button>
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

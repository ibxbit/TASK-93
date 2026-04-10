/**
 * AssetTable — rendering & pagination
 *
 * Tests:
 *  1. Renders a row for each asset passed in.
 *  2. Formats the status badge correctly.
 *  3. Hides pagination when totalPages === 1.
 *  4. Renders pagination and calls onPageChange correctly.
 */

import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import AssetTable from "@/components/assets/AssetTable";
import type { Asset } from "@/types";

const MOCK_ASSETS: Asset[] = [
  {
    id: 1,
    asset_code: "ASSET-2024-001",
    category: "vehicle",
    brand: "Toyota",
    model: "GR Yaris",
    status: "in_service",
    procurement_cost: "45000.00",
    procurement_date: "2024-01-15",
    useful_life_months: 60,
    notes: null,
    created_at: "2024-01-15T10:00:00Z",
    updated_at: "2024-01-15T10:00:00Z",
  },
  {
    id: 2,
    asset_code: "ASSET-2024-002",
    category: "equipment",
    brand: "Snap-on",
    model: "Torque Wrench Pro",
    status: "out_for_repair",
    procurement_cost: null,
    procurement_date: null,
    useful_life_months: null,
    notes: "Sent for calibration",
    created_at: "2024-02-01T08:00:00Z",
    updated_at: "2024-03-10T14:30:00Z",
  },
];

describe("AssetTable", () => {
  it("renders a row for each asset", () => {
    render(
      <AssetTable
        assets={MOCK_ASSETS}
        page={1}
        totalPages={1}
        onPageChange={jest.fn()}
      />
    );

    expect(screen.getByText("ASSET-2024-001")).toBeInTheDocument();
    expect(screen.getByText("ASSET-2024-002")).toBeInTheDocument();
    expect(screen.getByText("Toyota")).toBeInTheDocument();
    expect(screen.getByText("Snap-on")).toBeInTheDocument();
  });

  it("shows 'In Service' badge for in_service status", () => {
    render(
      <AssetTable
        assets={[MOCK_ASSETS[0]]}
        page={1}
        totalPages={1}
        onPageChange={jest.fn()}
      />
    );
    expect(screen.getByText("In Service")).toBeInTheDocument();
  });

  it("shows 'Out for Repair' badge for out_for_repair status", () => {
    render(
      <AssetTable
        assets={[MOCK_ASSETS[1]]}
        page={1}
        totalPages={1}
        onPageChange={jest.fn()}
      />
    );
    expect(screen.getByText("Out for Repair")).toBeInTheDocument();
  });

  it("does NOT render pagination when totalPages is 1", () => {
    render(
      <AssetTable
        assets={MOCK_ASSETS}
        page={1}
        totalPages={1}
        onPageChange={jest.fn()}
      />
    );
    expect(screen.queryByLabelText("Pagination")).not.toBeInTheDocument();
  });

  it("renders pagination and calls onPageChange on Next click", () => {
    const onPageChange = jest.fn();
    render(
      <AssetTable
        assets={MOCK_ASSETS}
        page={1}
        totalPages={3}
        onPageChange={onPageChange}
      />
    );

    expect(screen.getByLabelText("Pagination")).toBeInTheDocument();
    expect(screen.getByText("Page 1 of 3")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /next page/i }));
    expect(onPageChange).toHaveBeenCalledWith(2);
  });

  it("Prev button is disabled on the first page", () => {
    render(
      <AssetTable
        assets={MOCK_ASSETS}
        page={1}
        totalPages={3}
        onPageChange={jest.fn()}
      />
    );
    expect(screen.getByRole("button", { name: /previous page/i })).toBeDisabled();
  });

  it("Next button is disabled on the last page", () => {
    render(
      <AssetTable
        assets={MOCK_ASSETS}
        page={3}
        totalPages={3}
        onPageChange={jest.fn()}
      />
    );
    expect(screen.getByRole("button", { name: /next page/i })).toBeDisabled();
  });
});

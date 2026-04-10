/**
 * AssetFormModal — create & update-status modes
 *
 * Tests:
 *  1.  Renders nothing when isOpen is false.
 *  2.  [Create] renders all required fields when open.
 *  3.  [Create] submit button is disabled when required fields are empty.
 *  4.  [Create] submit button is enabled once all required fields are filled.
 *  5.  [Create] calls assetsService.create and onSuccess on success.
 *  6.  [Create] shows an ErrorBanner when the API call fails.
 *  7.  [Create] disables the submit button while submitting.
 *  8.  [Update-status] renders context note with asset details.
 *  9.  [Update-status] calls assetsService.updateStatus and onSuccess.
 * 10.  [Update-status] shows an ErrorBanner on failure.
 */

import React from "react";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import AssetFormModal from "@/components/assets/AssetFormModal";
import { ToastContext } from "@/context/ToastContext";
import type { Asset } from "@/types";

// ── Service mock ───────────────────────────────────────────────────────────────

jest.mock("@/services/assets.service", () => ({
  assetsService: {
    create: jest.fn(),
    updateStatus: jest.fn(),
  },
}));

import { assetsService } from "@/services/assets.service";
const mockCreate = assetsService.create as jest.Mock;
const mockUpdateStatus = assetsService.updateStatus as jest.Mock;

// ── Toast mock ────────────────────────────────────────────────────────────────

const mockAddToast = jest.fn();
function MockToastProvider({ children }: { children: React.ReactNode }) {
  return (
    <ToastContext.Provider
      value={{
        toasts: [],
        addToast: mockAddToast,
        removeToast: jest.fn(),
      }}
    >
      {children}
    </ToastContext.Provider>
  );
}

// ── Fixtures ──────────────────────────────────────────────────────────────────

const MOCK_ASSET: Asset = {
  id: 7,
  asset_code: "ASSET-2024-007",
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
};

function renderCreate(
  isOpen: boolean,
  onClose = jest.fn(),
  onSuccess = jest.fn()
) {
  return render(
    <MockToastProvider>
      <AssetFormModal
        isOpen={isOpen}
        onClose={onClose}
        mode="create"
        onSuccess={onSuccess}
      />
    </MockToastProvider>
  );
}

function renderUpdateStatus(
  isOpen: boolean,
  asset: Asset = MOCK_ASSET,
  onClose = jest.fn(),
  onSuccess = jest.fn()
) {
  return render(
    <MockToastProvider>
      <AssetFormModal
        isOpen={isOpen}
        onClose={onClose}
        mode="update-status"
        asset={asset}
        onSuccess={onSuccess}
      />
    </MockToastProvider>
  );
}

beforeEach(() => {
  mockCreate.mockReset();
  mockUpdateStatus.mockReset();
  mockAddToast.mockReset();
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("AssetFormModal — create mode", () => {
  it("renders nothing when isOpen is false", () => {
    const { container } = renderCreate(false);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders the form with required fields when open", () => {
    renderCreate(true);
    expect(screen.getByLabelText(/asset code/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/brand/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/model/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /register asset/i })).toBeInTheDocument();
  });

  it("disables submit when required fields are empty", () => {
    renderCreate(true);
    expect(
      screen.getByRole("button", { name: /register asset/i })
    ).toBeDisabled();
  });

  it("enables submit once all required fields are filled", async () => {
    renderCreate(true);
    await userEvent.type(screen.getByLabelText(/asset code/i), "ASSET-001");
    await userEvent.type(screen.getByLabelText(/brand/i), "Toyota");
    await userEvent.type(screen.getByLabelText(/model/i), "GR86");
    expect(
      screen.getByRole("button", { name: /register asset/i })
    ).not.toBeDisabled();
  });

  it("calls assetsService.create and onSuccess on happy path", async () => {
    const onSuccess = jest.fn();
    mockCreate.mockResolvedValue({ ...MOCK_ASSET, asset_code: "ASSET-NEW" });

    renderCreate(true, jest.fn(), onSuccess);

    await userEvent.type(screen.getByLabelText(/asset code/i), "ASSET-NEW");
    await userEvent.type(screen.getByLabelText(/brand/i), "Honda");
    await userEvent.type(screen.getByLabelText(/model/i), "Civic Type-R");

    fireEvent.click(screen.getByRole("button", { name: /register asset/i }));

    await waitFor(() => expect(mockCreate).toHaveBeenCalledTimes(1));
    expect(mockCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        asset_code: "ASSET-NEW",
        brand: "Honda",
        model: "Civic Type-R",
      })
    );
    await waitFor(() => expect(onSuccess).toHaveBeenCalledTimes(1));
  });

  it("shows an ErrorBanner when create fails", async () => {
    mockCreate.mockRejectedValue(new Error("Duplicate asset code."));

    renderCreate(true);

    await userEvent.type(screen.getByLabelText(/asset code/i), "DUPE-001");
    await userEvent.type(screen.getByLabelText(/brand/i), "Ford");
    await userEvent.type(screen.getByLabelText(/model/i), "Mustang");

    fireEvent.click(screen.getByRole("button", { name: /register asset/i }));

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        /duplicate asset code/i
      )
    );
  });

  it("disables the submit button while submitting (loading state)", async () => {
    let resolveCreate!: (v: Asset) => void;
    mockCreate.mockImplementation(
      () => new Promise<Asset>((res) => { resolveCreate = res; })
    );

    renderCreate(true);

    await userEvent.type(screen.getByLabelText(/asset code/i), "ASSET-SLOW");
    await userEvent.type(screen.getByLabelText(/brand/i), "BMW");
    await userEvent.type(screen.getByLabelText(/model/i), "M3");

    fireEvent.click(screen.getByRole("button", { name: /register asset/i }));

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /registering/i })
      ).toBeDisabled()
    );

    resolveCreate(MOCK_ASSET);
  });
});

describe("AssetFormModal — update-status mode", () => {
  it("renders the asset context note with asset details", () => {
    renderUpdateStatus(true);
    expect(screen.getByText(/ASSET-2024-007/)).toBeInTheDocument();
    expect(screen.getByText(/Toyota/)).toBeInTheDocument();
  });

  it("calls assetsService.updateStatus and onSuccess on happy path", async () => {
    const onSuccess = jest.fn();
    const updated = { ...MOCK_ASSET, status: "out_for_repair" as const };
    mockUpdateStatus.mockResolvedValue(updated);

    renderUpdateStatus(true, MOCK_ASSET, jest.fn(), onSuccess);

    // Change status from in_service → out_for_repair
    fireEvent.change(screen.getByLabelText(/new status/i), {
      target: { value: "out_for_repair" },
    });

    fireEvent.click(screen.getByRole("button", { name: /save status/i }));

    await waitFor(() => expect(mockUpdateStatus).toHaveBeenCalledTimes(1));
    expect(mockUpdateStatus).toHaveBeenCalledWith(7, {
      status: "out_for_repair",
    });
    await waitFor(() => expect(onSuccess).toHaveBeenCalledTimes(1));
  });

  it("shows an ErrorBanner when updateStatus fails", async () => {
    mockUpdateStatus.mockRejectedValue(
      new Error("Invalid status transition.")
    );

    renderUpdateStatus(true);

    fireEvent.change(screen.getByLabelText(/new status/i), {
      target: { value: "retired" },
    });
    fireEvent.click(screen.getByRole("button", { name: /save status/i }));

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        /invalid status transition/i
      )
    );
  });
});

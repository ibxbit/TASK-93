/**
 * Invoices UI — InvoicesTable, InvoicesSearch, InvoiceFormModal
 *
 * Tests:
 *  InvoicesTable
 *  1.  Renders one row per invoice.
 *  2.  Displays correct status badge labels (Draft, Issued, Paid, Overdue, Cancelled).
 *  3.  Hides pagination when totalPages === 1.
 *  4.  Shows pagination controls when totalPages > 1.
 *  5.  Calls onPageChange with page - 1 when Prev is clicked.
 *  6.  Calls onPageChange with page + 1 when Next is clicked.
 *  7.  Prev button is disabled on the first page.
 *  8.  Next button is disabled on the last page.
 *  9.  "Issue" button only appears for draft invoices when onIssue is provided.
 * 10.  "Issue" button is NOT rendered for non-draft invoices.
 * 11.  Calls onIssue with the correct invoice when "Issue" is clicked.
 * 12.  Actions column is absent when onIssue is not provided.
 *
 *  InvoicesSearch
 * 13.  Calls onSearch when the text input changes.
 * 14.  Calls onStatus when the status select changes.
 *
 *  InvoiceFormModal
 * 15.  Renders nothing when isOpen is false.
 * 16.  Renders the counterparty and issue-date fields when open.
 * 17.  Submit button is disabled when counterparty is empty.
 * 18.  Submit button is enabled once counterparty is filled.
 * 19.  Calls invoicesService.create and onCreated on success.
 * 20.  Shows an ErrorBanner when the API call fails.
 */

import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import InvoicesTable from "@/components/invoices/InvoicesTable";
import InvoicesSearch from "@/components/invoices/InvoicesSearch";
import InvoiceFormModal from "@/components/invoices/InvoiceFormModal";
import { ToastContext } from "@/context/ToastContext";
import type { Invoice } from "@/types";

// ── Service mock ───────────────────────────────────────────────────────────────

jest.mock("@/services/invoices.service", () => ({
  invoicesService: {
    create: jest.fn(),
    issue: jest.fn(),
  },
}));

import { invoicesService } from "@/services/invoices.service";
const mockCreate = invoicesService.create as jest.Mock;

// ── Toast mock ─────────────────────────────────────────────────────────────────

const mockAddToast = jest.fn();
function MockToastProvider({ children }: { children: React.ReactNode }) {
  return (
    <ToastContext.Provider
      value={{ toasts: [], addToast: mockAddToast, removeToast: jest.fn() }}
    >
      {children}
    </ToastContext.Provider>
  );
}

// ── Fixtures ───────────────────────────────────────────────────────────────────

function makeInvoice(overrides: Partial<Invoice> = {}): Invoice {
  return {
    id: 1,
    invoice_no: "INV-2024-0001",
    counterparty: "Acme Racing Pty Ltd",
    issue_date: "2024-03-15",
    tax_rate: "0.1000",
    subtotal: "1000.00",
    tax: "100.00",
    discount_amount: "0.00",
    total: "1100.00",
    status: "draft",
    created_by: 1,
    created_at: "2024-03-15T10:00:00Z",
    updated_at: "2024-03-15T10:00:00Z",
    ...overrides,
  };
}

const DRAFT_INVOICE = makeInvoice({ id: 1, status: "draft", invoice_no: "INV-2024-0001" });
const ISSUED_INVOICE = makeInvoice({ id: 2, status: "issued", invoice_no: "INV-2024-0002" });
const PAID_INVOICE = makeInvoice({ id: 3, status: "paid", invoice_no: "INV-2024-0003" });
const OVERDUE_INVOICE = makeInvoice({ id: 4, status: "overdue", invoice_no: "INV-2024-0004" });
const CANCELLED_INVOICE = makeInvoice({ id: 5, status: "cancelled", invoice_no: "INV-2024-0005" });

beforeEach(() => {
  mockCreate.mockReset();
  mockAddToast.mockReset();
});

// ── InvoicesTable ──────────────────────────────────────────────────────────────

describe("InvoicesTable", () => {
  const invoices = [DRAFT_INVOICE, ISSUED_INVOICE];

  function renderTable(
    invs: Invoice[] = invoices,
    page = 1,
    totalPages = 1,
    onPageChange = jest.fn(),
    onIssue?: (inv: Invoice) => void
  ) {
    return render(
      <InvoicesTable
        invoices={invs}
        page={page}
        totalPages={totalPages}
        onPageChange={onPageChange}
        onIssue={onIssue}
      />
    );
  }

  it("renders one row per invoice", () => {
    renderTable();
    expect(screen.getByText("INV-2024-0001")).toBeInTheDocument();
    expect(screen.getByText("INV-2024-0002")).toBeInTheDocument();
  });

  it("displays correct status badge labels", () => {
    renderTable([
      DRAFT_INVOICE,
      ISSUED_INVOICE,
      PAID_INVOICE,
      OVERDUE_INVOICE,
      CANCELLED_INVOICE,
    ]);
    expect(screen.getByTestId("inv-status-1")).toHaveTextContent("Draft");
    expect(screen.getByTestId("inv-status-2")).toHaveTextContent("Issued");
    expect(screen.getByTestId("inv-status-3")).toHaveTextContent("Paid");
    expect(screen.getByTestId("inv-status-4")).toHaveTextContent("Overdue");
    expect(screen.getByTestId("inv-status-5")).toHaveTextContent("Cancelled");
  });

  it("hides pagination when totalPages === 1", () => {
    renderTable(invoices, 1, 1);
    expect(screen.queryByLabelText("Pagination")).not.toBeInTheDocument();
  });

  it("shows pagination controls when totalPages > 1", () => {
    renderTable(invoices, 1, 3);
    expect(screen.getByLabelText("Pagination")).toBeInTheDocument();
    expect(screen.getByText(/page 1 of 3/i)).toBeInTheDocument();
  });

  it("calls onPageChange with page - 1 when Prev is clicked", () => {
    const onPageChange = jest.fn();
    renderTable(invoices, 2, 3, onPageChange);
    fireEvent.click(screen.getByRole("button", { name: /previous page/i }));
    expect(onPageChange).toHaveBeenCalledWith(1);
  });

  it("calls onPageChange with page + 1 when Next is clicked", () => {
    const onPageChange = jest.fn();
    renderTable(invoices, 1, 3, onPageChange);
    fireEvent.click(screen.getByRole("button", { name: /next page/i }));
    expect(onPageChange).toHaveBeenCalledWith(2);
  });

  it("disables Prev button on the first page", () => {
    renderTable(invoices, 1, 3);
    expect(screen.getByRole("button", { name: /previous page/i })).toBeDisabled();
  });

  it("disables Next button on the last page", () => {
    renderTable(invoices, 3, 3);
    expect(screen.getByRole("button", { name: /next page/i })).toBeDisabled();
  });

  it("renders Issue button only for draft invoices when onIssue provided", () => {
    renderTable([DRAFT_INVOICE, ISSUED_INVOICE], 1, 1, jest.fn(), jest.fn());
    expect(
      screen.getByRole("button", { name: /issue invoice INV-2024-0001/i })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /issue invoice INV-2024-0002/i })
    ).not.toBeInTheDocument();
  });

  it("does not render Issue button for non-draft invoices", () => {
    renderTable([PAID_INVOICE, CANCELLED_INVOICE], 1, 1, jest.fn(), jest.fn());
    expect(screen.queryByRole("button", { name: /issue invoice/i })).not.toBeInTheDocument();
  });

  it("calls onIssue with the correct invoice when Issue is clicked", () => {
    const onIssue = jest.fn();
    renderTable([DRAFT_INVOICE], 1, 1, jest.fn(), onIssue);
    fireEvent.click(
      screen.getByRole("button", { name: /issue invoice INV-2024-0001/i })
    );
    expect(onIssue).toHaveBeenCalledWith(DRAFT_INVOICE);
  });

  it("omits the Actions column when onIssue is not provided", () => {
    renderTable(invoices, 1, 1, jest.fn(), undefined);
    expect(screen.queryByText("Actions")).not.toBeInTheDocument();
  });
});

// ── InvoicesSearch ─────────────────────────────────────────────────────────────

describe("InvoicesSearch", () => {
  it("calls onSearch when the text input changes", async () => {
    const onSearch = jest.fn();
    render(
      <InvoicesSearch
        search=""
        onSearch={onSearch}
        status=""
        onStatus={jest.fn()}
      />
    );
    await userEvent.type(screen.getByLabelText(/search invoices/i), "Acme");
    expect(onSearch).toHaveBeenCalled();
    expect(onSearch).toHaveBeenLastCalledWith(expect.stringContaining("A"));
  });

  it("calls onStatus when the status select changes", () => {
    const onStatus = jest.fn();
    render(
      <InvoicesSearch
        search=""
        onSearch={jest.fn()}
        status=""
        onStatus={onStatus}
      />
    );
    fireEvent.change(screen.getByLabelText(/filter by status/i), {
      target: { value: "paid" },
    });
    expect(onStatus).toHaveBeenCalledWith("paid");
  });
});

// ── InvoiceFormModal ───────────────────────────────────────────────────────────

describe("InvoiceFormModal", () => {
  function renderModal(
    isOpen: boolean,
    onClose = jest.fn(),
    onCreated = jest.fn()
  ) {
    return render(
      <MockToastProvider>
        <InvoiceFormModal
          isOpen={isOpen}
          onClose={onClose}
          onCreated={onCreated}
        />
      </MockToastProvider>
    );
  }

  it("renders nothing when isOpen is false", () => {
    const { container } = renderModal(false);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders counterparty and issue-date fields when open", () => {
    renderModal(true);
    expect(screen.getByLabelText(/counterparty/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/issue date/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /create invoice/i })
    ).toBeInTheDocument();
  });

  it("disables submit when counterparty is empty", () => {
    renderModal(true);
    expect(
      screen.getByRole("button", { name: /create invoice/i })
    ).toBeDisabled();
  });

  it("enables submit once counterparty is filled", async () => {
    renderModal(true);
    await userEvent.type(
      screen.getByLabelText(/counterparty/i),
      "Acme Racing Pty Ltd"
    );
    expect(
      screen.getByRole("button", { name: /create invoice/i })
    ).not.toBeDisabled();
  });

  it("calls invoicesService.create and onCreated on success", async () => {
    const onCreated = jest.fn();
    const created = { ...DRAFT_INVOICE, invoice_no: "INV-2024-0099" };
    mockCreate.mockResolvedValue(created);

    renderModal(true, jest.fn(), onCreated);

    await userEvent.type(
      screen.getByLabelText(/counterparty/i),
      "Apex Motorsport"
    );

    fireEvent.click(screen.getByRole("button", { name: /create invoice/i }));

    await waitFor(() => expect(mockCreate).toHaveBeenCalledTimes(1));
    expect(mockCreate).toHaveBeenCalledWith(
      expect.objectContaining({ counterparty: "Apex Motorsport" })
    );
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(created));
  });

  it("shows an ErrorBanner when the API call fails", async () => {
    mockCreate.mockRejectedValue(new Error("Counterparty name too long."));

    renderModal(true);

    await userEvent.type(
      screen.getByLabelText(/counterparty/i),
      "A".repeat(300)
    );

    fireEvent.click(screen.getByRole("button", { name: /create invoice/i }));

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        /counterparty name too long/i
      )
    );
  });
});

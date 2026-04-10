/**
 * EventsTable + EventsSearch — rendering, filtering, and state coverage
 *
 * Covers:
 *  1. EventsTable: renders a row for each event.
 *  2. EventsTable: displays correct status badge text.
 *  3. EventsTable: shows championship check mark vs dash.
 *  4. EventsTable: hides pagination when totalPages === 1.
 *  5. EventsTable: shows pagination and calls onPageChange.
 *  6. EventsTable: Prev/Next disabled at boundary pages.
 *  7. EventsSearch: calls onSearch when the user types.
 *  8. EventsSearch: calls onStatus when the user changes the select.
 *  9. EmptyState: shows "No events found" message for the Results page empty case.
 * 10. ErrorBanner: shows the error message and fires onRetry.
 */

import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import EventsTable from "@/components/events/EventsTable";
import EventsSearch from "@/components/events/EventsSearch";
import EmptyState from "@/components/ui/EmptyState";
import ErrorBanner from "@/components/ui/ErrorBanner";
import type { MotorsportEvent } from "@/types";

// ── Fixtures ──────────────────────────────────────────────────────────────────

const MOCK_EVENTS: MotorsportEvent[] = [
  {
    id: 1,
    name: "Round 1 — Monza",
    description: null,
    venue_identifier: "VENUE-MONZA-01",
    schedule_group: "2024-Sprint",
    status: "completed",
    is_championship_class: true,
    created_at: "2024-03-01T08:00:00Z",
    updated_at: "2024-03-15T18:00:00Z",
  },
  {
    id: 2,
    name: "Round 2 — Silverstone",
    description: "Second round of the season",
    venue_identifier: "VENUE-SILVERSTONE-01",
    schedule_group: "2024-Sprint",
    status: "in_progress",
    is_championship_class: false,
    created_at: "2024-04-01T08:00:00Z",
    updated_at: "2024-04-10T12:00:00Z",
  },
  {
    id: 3,
    name: "Test Session — Nürburgring",
    description: "Pre-season shakedown",
    venue_identifier: "VENUE-NURBURGRING-01",
    schedule_group: null,
    status: "draft",
    is_championship_class: false,
    created_at: "2024-01-15T10:00:00Z",
    updated_at: "2024-01-15T10:00:00Z",
  },
];

// ── EventsTable ───────────────────────────────────────────────────────────────

describe("EventsTable", () => {
  function renderTable(
    overrides: Partial<React.ComponentProps<typeof EventsTable>> = {}
  ) {
    return render(
      <EventsTable
        events={MOCK_EVENTS}
        page={1}
        totalPages={1}
        onPageChange={jest.fn()}
        {...overrides}
      />
    );
  }

  it("renders a row for every event passed in", () => {
    renderTable();
    expect(screen.getByText("Round 1 — Monza")).toBeInTheDocument();
    expect(screen.getByText("Round 2 — Silverstone")).toBeInTheDocument();
    expect(screen.getByText("Test Session — Nürburgring")).toBeInTheDocument();
  });

  it("renders venue identifiers", () => {
    renderTable();
    expect(screen.getByText("VENUE-MONZA-01")).toBeInTheDocument();
  });

  it("shows '—' for missing schedule group", () => {
    // Event 3 has schedule_group: null
    renderTable({ events: [MOCK_EVENTS[2]] });
    // There will be a '—' in the schedule_group column
    expect(screen.getAllByText("—").length).toBeGreaterThan(0);
  });

  it("displays 'Completed' status badge", () => {
    renderTable({ events: [MOCK_EVENTS[0]] });
    expect(screen.getByText("Completed")).toBeInTheDocument();
  });

  it("displays 'In Progress' status badge", () => {
    renderTable({ events: [MOCK_EVENTS[1]] });
    expect(screen.getByText("In Progress")).toBeInTheDocument();
  });

  it("displays 'Draft' status badge", () => {
    renderTable({ events: [MOCK_EVENTS[2]] });
    expect(screen.getByText("Draft")).toBeInTheDocument();
  });

  it("shows championship check (✓) for championship events", () => {
    renderTable({ events: [MOCK_EVENTS[0]] });
    expect(screen.getByLabelText("Yes")).toBeInTheDocument();
  });

  it("shows championship dash (—) for non-championship events", () => {
    renderTable({ events: [MOCK_EVENTS[1]] });
    expect(screen.getByLabelText("No")).toBeInTheDocument();
  });

  it("does NOT render pagination when totalPages is 1", () => {
    renderTable({ totalPages: 1 });
    expect(screen.queryByLabelText("Pagination")).not.toBeInTheDocument();
  });

  it("renders pagination controls when totalPages > 1", () => {
    renderTable({ totalPages: 4, page: 2 });
    expect(screen.getByLabelText("Pagination")).toBeInTheDocument();
    expect(screen.getByText("Page 2 of 4")).toBeInTheDocument();
  });

  it("calls onPageChange(page + 1) when Next is clicked", () => {
    const onPageChange = jest.fn();
    renderTable({ page: 1, totalPages: 3, onPageChange });
    fireEvent.click(screen.getByRole("button", { name: /next page/i }));
    expect(onPageChange).toHaveBeenCalledWith(2);
  });

  it("calls onPageChange(page - 1) when Prev is clicked", () => {
    const onPageChange = jest.fn();
    renderTable({ page: 3, totalPages: 3, onPageChange });
    fireEvent.click(screen.getByRole("button", { name: /previous page/i }));
    expect(onPageChange).toHaveBeenCalledWith(2);
  });

  it("disables Prev on the first page", () => {
    renderTable({ page: 1, totalPages: 3 });
    expect(screen.getByRole("button", { name: /previous page/i })).toBeDisabled();
  });

  it("disables Next on the last page", () => {
    renderTable({ page: 3, totalPages: 3 });
    expect(screen.getByRole("button", { name: /next page/i })).toBeDisabled();
  });
});

// ── EventsSearch ──────────────────────────────────────────────────────────────

describe("EventsSearch", () => {
  it("calls onSearch when the user types into the search input", async () => {
    const onSearch = jest.fn();
    render(
      <EventsSearch
        search=""
        onSearch={onSearch}
        status=""
        onStatus={jest.fn()}
      />
    );

    await userEvent.type(screen.getByRole("searchbox"), "Monza");
    // userEvent.type fires a change per character
    expect(onSearch).toHaveBeenCalled();
    expect(onSearch).toHaveBeenLastCalledWith("Monza");
  });

  it("calls onStatus when the user changes the status select", () => {
    const onStatus = jest.fn();
    render(
      <EventsSearch
        search=""
        onSearch={jest.fn()}
        status=""
        onStatus={onStatus}
      />
    );

    fireEvent.change(screen.getByLabelText(/filter by status/i), {
      target: { value: "completed" },
    });
    expect(onStatus).toHaveBeenCalledWith("completed");
  });

  it("renders all status options", () => {
    render(
      <EventsSearch
        search=""
        onSearch={jest.fn()}
        status=""
        onStatus={jest.fn()}
      />
    );
    expect(screen.getByRole("option", { name: "All statuses" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Draft" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Published" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "In Progress" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Completed" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Cancelled" })).toBeInTheDocument();
  });
});

// ── EmptyState (Results page context) ─────────────────────────────────────────

describe("EmptyState — Results page variants", () => {
  it("renders the 'No events found' message for empty results with active filter", () => {
    render(
      <EmptyState
        title="No events found"
        message="Try clearing your search or status filter."
      />
    );
    expect(screen.getByText("No events found")).toBeInTheDocument();
    expect(
      screen.getByText("Try clearing your search or status filter.")
    ).toBeInTheDocument();
  });

  it("renders the 'no events created yet' message for empty results without filter", () => {
    render(
      <EmptyState
        title="No events found"
        message="No motorsport events have been created yet."
      />
    );
    expect(
      screen.getByText("No motorsport events have been created yet.")
    ).toBeInTheDocument();
  });
});

// ── ErrorBanner ────────────────────────────────────────────────────────────────

describe("ErrorBanner", () => {
  it("renders the error message", () => {
    render(<ErrorBanner message="Failed to load events." />);
    expect(screen.getByRole("alert")).toHaveTextContent("Failed to load events.");
  });

  it("does not render a Retry button when onRetry is not provided", () => {
    render(<ErrorBanner message="Something went wrong." />);
    expect(screen.queryByRole("button", { name: /retry/i })).not.toBeInTheDocument();
  });

  it("renders a Retry button and fires onRetry when clicked", () => {
    const onRetry = jest.fn();
    render(<ErrorBanner message="Network error." onRetry={onRetry} />);
    fireEvent.click(screen.getByRole("button", { name: /retry/i }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });
});

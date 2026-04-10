/**
 * RoleNav — role-based conditional rendering
 *
 * Tests:
 *  1. Shows "Asset Register" link to all authenticated users.
 *  2. Shows "Administration" link only to Administrators.
 *  3. Hides "Administration" for non-admin roles.
 *  4. Shows "Invoices" link for FinanceClerk.
 *  5. Displays the user's role pills.
 *  6. EmptyState renders with correct text.
 */

import React from "react";
import { render, screen } from "@testing-library/react";
import RoleNav from "@/components/dashboard/RoleNav";
import EmptyState from "@/components/ui/EmptyState";
import type { Role } from "@/types";

describe("RoleNav", () => {
  it("shows Asset Register to a user with no roles", () => {
    render(<RoleNav userRoles={[]} />);
    expect(screen.getByText("Asset Register")).toBeInTheDocument();
  });

  it("shows Events & Results link to EventDirector", () => {
    render(<RoleNav userRoles={["EventDirector"]} />);
    expect(screen.getByText("Events & Results")).toBeInTheDocument();
  });

  it("shows Administration link to Administrator", () => {
    render(<RoleNav userRoles={["Administrator"]} />);
    expect(screen.getByText("Administration")).toBeInTheDocument();
  });

  it("hides Administration for Referee", () => {
    render(<RoleNav userRoles={["Referee"]} />);
    expect(screen.queryByText("Administration")).not.toBeInTheDocument();
  });

  it("shows Invoices link for FinanceClerk", () => {
    render(<RoleNav userRoles={["FinanceClerk"]} />);
    expect(screen.getByText("Invoices")).toBeInTheDocument();
  });

  it("hides Invoices for Referee", () => {
    render(<RoleNav userRoles={["Referee"]} />);
    expect(screen.queryByText("Invoices")).not.toBeInTheDocument();
  });

  it("shows Audit Logs for Auditor", () => {
    render(<RoleNav userRoles={["Auditor"]} />);
    expect(screen.getByText("Audit Logs")).toBeInTheDocument();
  });

  it("Events & Results is hidden for FinanceClerk", () => {
    render(<RoleNav userRoles={["FinanceClerk"]} />);
    expect(screen.queryByText("Events & Results")).not.toBeInTheDocument();
  });

  it("displays role pills for each assigned role", () => {
    const roles: Role[] = ["Administrator", "EventDirector"];
    render(<RoleNav userRoles={roles} />);
    expect(screen.getByText("Administrator")).toBeInTheDocument();
    expect(screen.getByText("Event Director")).toBeInTheDocument();
  });
});

describe("EmptyState", () => {
  it("renders the default title and message", () => {
    render(<EmptyState />);
    expect(screen.getByText("No records found")).toBeInTheDocument();
    expect(
      screen.getByText(/try adjusting your filters/i)
    ).toBeInTheDocument();
  });

  it("renders custom title and message", () => {
    render(
      <EmptyState
        title="No assets found"
        message="Clear your filters to see all assets."
      />
    );
    expect(screen.getByText("No assets found")).toBeInTheDocument();
    expect(
      screen.getByText("Clear your filters to see all assets.")
    ).toBeInTheDocument();
  });
});

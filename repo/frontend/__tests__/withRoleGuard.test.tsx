/**
 * withRoleGuard HOC
 *
 * Tests:
 *  1.  Shows a spinner while isLoading is true.
 *  2.  Renders nothing (redirect in flight) when user is null after loading.
 *  3.  Calls router.replace("/login") when not authenticated.
 *  4.  Renders nothing when authenticated but holds no allowed role.
 *  5.  Calls router.replace("/dashboard") when the user lacks required roles.
 *  6.  Calls router.replace(custom path) when redirectTo is overridden.
 *  7.  Renders the wrapped page when the user holds an allowed role.
 *  8.  Renders the wrapped page for allowedRoles=[] (any authenticated user).
 *  9.  Sets displayName correctly on the guarded component.
 */

import React from "react";
import { render, screen } from "@testing-library/react";
import { withRoleGuard } from "@/components/auth/withRoleGuard";

// ── Mocks ─────────────────────────────────────────────────────────────────────

const mockRouterReplace = jest.fn();

jest.mock("next/router", () => ({
  useRouter: () => ({ replace: mockRouterReplace }),
}));

jest.mock("@/context/AuthContext", () => ({
  useAuth: jest.fn(),
}));

// Import the mock so we can control its return value per-test
import { useAuth } from "@/context/AuthContext";
const mockUseAuth = useAuth as jest.Mock;

// ── Fixture component ─────────────────────────────────────────────────────────

function SecretPage() {
  return <div data-testid="secret-page">Secret Content</div>;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function makeUser(roles: string[] = []) {
  return { user_id: 1, username: "testuser", roles, token: "tok" };
}

beforeEach(() => {
  mockRouterReplace.mockClear();
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("withRoleGuard", () => {
  it("shows a spinner while auth is loading", () => {
    mockUseAuth.mockReturnValue({ user: null, isLoading: true });
    const Guarded = withRoleGuard(SecretPage, {
      allowedRoles: ["Administrator"],
    });

    render(<Guarded />);

    expect(screen.getByRole("status")).toBeInTheDocument();
    expect(screen.queryByTestId("secret-page")).not.toBeInTheDocument();
  });

  it("renders nothing (not the page) when unauthenticated after loading", () => {
    mockUseAuth.mockReturnValue({ user: null, isLoading: false });
    const Guarded = withRoleGuard(SecretPage, {
      allowedRoles: ["Administrator"],
    });

    const { container } = render(<Guarded />);

    expect(container).toBeEmptyDOMElement();
  });

  it("calls router.replace('/login') when not authenticated", () => {
    mockUseAuth.mockReturnValue({ user: null, isLoading: false });
    const Guarded = withRoleGuard(SecretPage, {
      allowedRoles: ["Administrator"],
    });

    render(<Guarded />);

    expect(mockRouterReplace).toHaveBeenCalledWith("/login");
  });

  it("renders nothing when authenticated but missing required role", () => {
    mockUseAuth.mockReturnValue({
      user: makeUser(["Referee"]),
      isLoading: false,
    });
    const Guarded = withRoleGuard(SecretPage, {
      allowedRoles: ["Administrator", "FinanceClerk"],
    });

    const { container } = render(<Guarded />);

    expect(container).toBeEmptyDOMElement();
    expect(screen.queryByTestId("secret-page")).not.toBeInTheDocument();
  });

  it("calls router.replace('/dashboard') by default when unauthorized", () => {
    mockUseAuth.mockReturnValue({
      user: makeUser(["Referee"]),
      isLoading: false,
    });
    const Guarded = withRoleGuard(SecretPage, {
      allowedRoles: ["Administrator"],
    });

    render(<Guarded />);

    expect(mockRouterReplace).toHaveBeenCalledWith("/dashboard");
  });

  it("calls router.replace with custom redirectTo path", () => {
    mockUseAuth.mockReturnValue({
      user: makeUser(["Referee"]),
      isLoading: false,
    });
    const Guarded = withRoleGuard(SecretPage, {
      allowedRoles: ["Administrator"],
      redirectTo: "/access-denied",
    });

    render(<Guarded />);

    expect(mockRouterReplace).toHaveBeenCalledWith("/access-denied");
  });

  it("renders the wrapped page when the user holds an allowed role", () => {
    mockUseAuth.mockReturnValue({
      user: makeUser(["FinanceClerk"]),
      isLoading: false,
    });
    const Guarded = withRoleGuard(SecretPage, {
      allowedRoles: ["Administrator", "FinanceClerk"],
    });

    render(<Guarded />);

    expect(screen.getByTestId("secret-page")).toBeInTheDocument();
    expect(mockRouterReplace).not.toHaveBeenCalled();
  });

  it("renders the wrapped page for allowedRoles=[] (any authenticated user)", () => {
    mockUseAuth.mockReturnValue({
      user: makeUser([]),   // no roles at all
      isLoading: false,
    });
    const Guarded = withRoleGuard(SecretPage, { allowedRoles: [] });

    render(<Guarded />);

    expect(screen.getByTestId("secret-page")).toBeInTheDocument();
  });

  it("sets a descriptive displayName on the guarded component", () => {
    const Guarded = withRoleGuard(SecretPage, {
      allowedRoles: ["Administrator"],
    });
    expect(Guarded.displayName).toBe("WithRoleGuard(SecretPage)");
  });
});

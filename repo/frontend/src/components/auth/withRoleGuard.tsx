import React, { useEffect } from "react";
import { useRouter } from "next/router";
import { useAuth } from "@/context/AuthContext";
import Spinner from "@/components/ui/Spinner";
import type { Role } from "@/types";

export interface RoleGuardOptions {
  /**
   * Roles allowed to view this page.
   * Empty array → any authenticated user may access.
   */
  allowedRoles: Role[];
  /**
   * Where to redirect an authenticated-but-unauthorized visitor.
   * Defaults to "/dashboard".
   */
  redirectTo?: string;
}

/**
 * withRoleGuard — Next.js Pages Router HOC.
 *
 * Wraps a page component and enforces three checks in order:
 *   1. Shows <Spinner> while auth is resolving (SSR-safe).
 *   2. Redirects to /login when the visitor is unauthenticated.
 *   3. Redirects to `redirectTo` (default /dashboard) when the authenticated
 *      visitor holds none of the `allowedRoles`.
 *   4. Renders the wrapped page when all checks pass.
 *
 * Usage:
 *   function InvoicesPage() { ... }
 *   export default withRoleGuard(InvoicesPage, {
 *     allowedRoles: ["Administrator", "FinanceClerk", "Auditor"],
 *   });
 */
export function withRoleGuard<P extends Record<string, unknown>>(
  WrappedPage: React.ComponentType<P>,
  options: RoleGuardOptions
) {
  const { allowedRoles, redirectTo = "/dashboard" } = options;

  function GuardedPage(props: P) {
    const { user, isLoading } = useAuth();
    const router = useRouter();

    const isAuthorized =
      allowedRoles.length === 0 ||
      (user !== null && allowedRoles.some((r) => user.roles.includes(r)));

    useEffect(() => {
      if (isLoading) return;

      if (!user) {
        void router.replace("/login");
        return;
      }

      if (!isAuthorized) {
        void router.replace(redirectTo);
      }
    }, [user, isLoading, isAuthorized, router]);

    // Still resolving auth state (e.g. rehydrating from sessionStorage)
    if (isLoading) {
      return <Spinner label="Checking permissions…" />;
    }

    // Not authenticated — redirect effect is in-flight, render nothing
    if (!user) return null;

    // Authenticated but unauthorized — redirect in-flight
    if (!isAuthorized) return null;

    return <WrappedPage {...props} />;
  }

  GuardedPage.displayName = `WithRoleGuard(${
    WrappedPage.displayName ?? WrappedPage.name ?? "Component"
  })`;

  return GuardedPage;
}

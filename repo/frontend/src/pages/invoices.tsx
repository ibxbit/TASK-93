import { useState } from "react";
import Head from "next/head";
import Layout from "@/components/layout/Layout";
import InvoicesSearch from "@/components/invoices/InvoicesSearch";
import InvoicesTable from "@/components/invoices/InvoicesTable";
import InvoiceFormModal from "@/components/invoices/InvoiceFormModal";
import Spinner from "@/components/ui/Spinner";
import EmptyState from "@/components/ui/EmptyState";
import ErrorBanner from "@/components/ui/ErrorBanner";
import { withRoleGuard } from "@/components/auth/withRoleGuard";
import { useToast } from "@/context/ToastContext";
import { invoicesService } from "@/services/invoices.service";
import { useInvoices } from "@/hooks/useInvoices";
import { useAuth } from "@/context/AuthContext";
import type { Invoice } from "@/types";
import styles from "./invoices.module.css";

function InvoicesPage() {
  const { user } = useAuth();
  const { addToast } = useToast();
  const [isCreateOpen, setIsCreateOpen] = useState(false);

  const {
    filtered,
    isLoading,
    error,
    search,
    setSearch,
    statusFilter,
    setStatusFilter,
    page,
    totalPages,
    setPage,
    reload,
  } = useInvoices();

  // Only FinanceClerk and Administrator can create invoices; Auditor is read-only
  const canCreate =
    user?.roles.includes("FinanceClerk") ||
    user?.roles.includes("Administrator");

  // Only FinanceClerk and Administrator can issue invoices
  const canIssue =
    user?.roles.includes("FinanceClerk") ||
    user?.roles.includes("Administrator");

  async function handleIssue(invoice: Invoice) {
    try {
      await invoicesService.issue(invoice.id);
      addToast(`Invoice ${invoice.invoice_no} issued.`, "success");
      reload();
    } catch (err: unknown) {
      addToast(
        err instanceof Error ? err.message : "Failed to issue invoice.",
        "error"
      );
    }
  }

  return (
    <>
      <Head>
        <title>Invoices — Motorsport Ops</title>
      </Head>
      <Layout>
        <div className={styles.header}>
          <div>
            <h1 className={styles.heading}>Invoices</h1>
            <p className={styles.sub}>
              Manage billing records, track payment status, and issue invoices
              to counterparties.
            </p>
          </div>
          {canCreate && (
            <button
              type="button"
              className={styles.createBtn}
              onClick={() => setIsCreateOpen(true)}
            >
              + Create Invoice
            </button>
          )}
        </div>

        <div className={styles.toolbar}>
          <InvoicesSearch
            search={search}
            onSearch={setSearch}
            status={statusFilter}
            onStatus={setStatusFilter}
          />
        </div>

        {error && (
          <div className={styles.errorWrapper}>
            <ErrorBanner message={error} onRetry={reload} />
          </div>
        )}

        {isLoading && <Spinner label="Loading invoices…" />}

        {!isLoading && !error && filtered.length === 0 && (
          <EmptyState
            title="No invoices found"
            message={
              search || statusFilter
                ? "Try clearing your search or status filter."
                : "No invoices have been created yet."
            }
          />
        )}

        {!isLoading && !error && filtered.length > 0 && (
          <InvoicesTable
            invoices={filtered}
            page={page}
            totalPages={totalPages}
            onPageChange={setPage}
            onIssue={canIssue ? handleIssue : undefined}
          />
        )}
      </Layout>

      <InvoiceFormModal
        isOpen={isCreateOpen}
        onClose={() => setIsCreateOpen(false)}
        onCreated={() => {
          setIsCreateOpen(false);
          reload();
        }}
      />
    </>
  );
}

export default withRoleGuard(InvoicesPage, {
  allowedRoles: ["Administrator", "FinanceClerk", "Auditor"],
});

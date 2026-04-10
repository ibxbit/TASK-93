import { useState } from "react";
import Head from "next/head";
import Layout from "@/components/layout/Layout";
import EventsSearch from "@/components/events/EventsSearch";
import EventsTable from "@/components/events/EventsTable";
import ResultFormModal from "@/components/results/ResultFormModal";
import Spinner from "@/components/ui/Spinner";
import EmptyState from "@/components/ui/EmptyState";
import ErrorBanner from "@/components/ui/ErrorBanner";
import { withRoleGuard } from "@/components/auth/withRoleGuard";
import { useAuth } from "@/context/AuthContext";
import { useEvents } from "@/hooks/useEvents";
import type { MotorsportEvent } from "@/types";
import styles from "./results.module.css";

function ResultsPage() {
  const { user } = useAuth();
  const [recordOpen, setRecordOpen] = useState(false);
  const [arbitrateEvent, setArbitrateEvent] =
    useState<MotorsportEvent | null>(null);
  const [arbitrateResultId, setArbitrateResultId] = useState<string>("");

  const {
    events,
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
  } = useEvents();

  // EventDirectors, Referees, and Administrators can record results
  const canRecord =
    user?.roles.includes("Administrator") ||
    user?.roles.includes("EventDirector") ||
    user?.roles.includes("Referee");

  // Only EventDirectors and Administrators can arbitrate
  const canArbitrate =
    user?.roles.includes("Administrator") ||
    user?.roles.includes("EventDirector");

  return (
    <>
      <Head>
        <title>Results — Motorsport Ops</title>
      </Head>
      <Layout>
        <div className={styles.header}>
          <div>
            <h1 className={styles.heading}>Events &amp; Results</h1>
            <p className={styles.sub}>
              Browse all motorsport events and capture timing or distance
              results.
            </p>
          </div>
          <div className={styles.headerActions}>
            {canRecord && (
              <button
                type="button"
                className={styles.actionBtn}
                onClick={() => setRecordOpen(true)}
              >
                + Record Result
              </button>
            )}
            {canArbitrate && (
              <button
                type="button"
                className={`${styles.actionBtn} ${styles.arbitrateBtn}`}
                onClick={() => {
                  /* open arbitrate with no pre-selection */
                  setArbitrateEvent(null);
                  setArbitrateResultId("");
                }}
                data-testid="open-arbitrate"
              >
                Arbitrate
              </button>
            )}
          </div>
        </div>

        <div className={styles.toolbar}>
          <EventsSearch
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

        {isLoading && <Spinner label="Loading events…" />}

        {!isLoading && !error && filtered.length === 0 && (
          <EmptyState
            title="No events found"
            message={
              search || statusFilter
                ? "Try clearing your search or status filter."
                : "No motorsport events have been created yet."
            }
          />
        )}

        {!isLoading && !error && filtered.length > 0 && (
          <EventsTable
            events={filtered}
            page={page}
            totalPages={totalPages}
            onPageChange={setPage}
          />
        )}

        {/* Arbitrate helper form — shown when canArbitrate clicked */}
        {canArbitrate && arbitrateEvent === null && arbitrateResultId !== "" && (
          <ResultFormModal
            isOpen
            onClose={() => setArbitrateResultId("")}
            mode="arbitrate"
            eventId={arbitrateEvent as unknown as number}
            resultId={parseInt(arbitrateResultId, 10)}
            onSuccess={reload}
          />
        )}
      </Layout>

      {/* Record Result modal */}
      <ResultFormModal
        isOpen={recordOpen}
        onClose={() => setRecordOpen(false)}
        mode="record"
        events={events}
        onSuccess={() => {
          setRecordOpen(false);
          reload();
        }}
      />

      {/* Arbitrate modal (triggered from event row — future: pass eventId + resultId) */}
    </>
  );
}

export default withRoleGuard(ResultsPage, { allowedRoles: [] });

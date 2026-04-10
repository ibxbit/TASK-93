import { useState } from "react";
import Head from "next/head";
import Layout from "@/components/layout/Layout";
import AssetSearch from "@/components/assets/AssetSearch";
import AssetTable from "@/components/assets/AssetTable";
import AssetFormModal from "@/components/assets/AssetFormModal";
import Spinner from "@/components/ui/Spinner";
import EmptyState from "@/components/ui/EmptyState";
import ErrorBanner from "@/components/ui/ErrorBanner";
import { withRoleGuard } from "@/components/auth/withRoleGuard";
import { useAuth } from "@/context/AuthContext";
import { useAssets } from "@/hooks/useAssets";
import type { Asset } from "@/types";
import styles from "./assets.module.css";

function AssetsPage() {
  const { user } = useAuth();
  const [createOpen, setCreateOpen] = useState(false);
  const [statusAsset, setStatusAsset] = useState<Asset | null>(null);

  const {
    filtered,
    isLoading,
    error,
    search,
    setSearch,
    categoryFilter,
    setCategoryFilter,
    statusFilter,
    setStatusFilter,
    page,
    totalPages,
    setPage,
    reload,
  } = useAssets();

  // Administrators and EventDirectors can register and update assets
  const canMutate =
    user?.roles.includes("Administrator") ||
    user?.roles.includes("EventDirector");

  return (
    <>
      <Head>
        <title>Assets — Motorsport Ops</title>
      </Head>
      <Layout>
        <div className={styles.header}>
          <h1 className={styles.heading}>Asset Register</h1>
          {canMutate && (
            <button
              type="button"
              className={styles.createBtn}
              onClick={() => setCreateOpen(true)}
            >
              + Register Asset
            </button>
          )}
        </div>

        <div className={styles.toolbar}>
          <AssetSearch
            search={search}
            onSearch={setSearch}
            category={categoryFilter}
            onCategory={setCategoryFilter}
            status={statusFilter}
            onStatus={setStatusFilter}
          />
        </div>

        {error && (
          <div className={styles.errorWrapper}>
            <ErrorBanner message={error} onRetry={reload} />
          </div>
        )}

        {isLoading && <Spinner label="Loading assets…" />}

        {!isLoading && !error && filtered.length === 0 && (
          <EmptyState
            title="No assets found"
            message={
              search || categoryFilter || statusFilter
                ? "Try clearing your filters."
                : "No assets have been registered yet."
            }
          />
        )}

        {!isLoading && !error && filtered.length > 0 && (
          <AssetTable
            assets={filtered}
            page={page}
            totalPages={totalPages}
            onPageChange={setPage}
            onUpdateStatus={canMutate ? (a) => setStatusAsset(a) : undefined}
          />
        )}
      </Layout>

      {/* Create modal */}
      <AssetFormModal
        isOpen={createOpen}
        onClose={() => setCreateOpen(false)}
        mode="create"
        onSuccess={() => {
          setCreateOpen(false);
          reload();
        }}
      />

      {/* Status-update modal */}
      <AssetFormModal
        isOpen={statusAsset !== null}
        onClose={() => setStatusAsset(null)}
        mode="update-status"
        asset={statusAsset ?? undefined}
        onSuccess={() => {
          setStatusAsset(null);
          reload();
        }}
      />
    </>
  );
}

export default withRoleGuard(AssetsPage, { allowedRoles: [] });

import { useCallback, useEffect, useRef, useState } from "react";
import { assetsService } from "@/services/assets.service";
import type { Asset, AssetCategory, AssetListParams, AssetStatus } from "@/types";

const PAGE_SIZE = 10;

interface UseAssetsReturn {
  assets: Asset[];
  filtered: Asset[];
  isLoading: boolean;
  error: string | null;
  search: string;
  setSearch: (v: string) => void;
  categoryFilter: AssetCategory | "";
  setCategoryFilter: (v: AssetCategory | "") => void;
  statusFilter: AssetStatus | "";
  setStatusFilter: (v: AssetStatus | "") => void;
  page: number;
  totalPages: number;
  setPage: (p: number) => void;
  reload: () => void;
}

export function useAssets(): UseAssetsReturn {
  const [assets, setAssets] = useState<Asset[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [categoryFilter, setCategoryFilter] = useState<AssetCategory | "">("");
  const [statusFilter, setStatusFilter] = useState<AssetStatus | "">("");
  const [page, setPage] = useState(1);
  const abortRef = useRef<AbortController | null>(null);

  const load = useCallback(async () => {
    abortRef.current?.abort();
    abortRef.current = new AbortController();

    setIsLoading(true);
    setError(null);

    const params: AssetListParams = {};
    if (categoryFilter) params.category = categoryFilter;
    if (statusFilter) params.status = statusFilter;

    try {
      const data = await assetsService.list(params);
      setAssets(data);
      setPage(1);
    } catch (err: unknown) {
      if (err instanceof Error && err.name !== "AbortError") {
        setError(err.message || "Failed to load assets.");
      }
    } finally {
      setIsLoading(false);
    }
  }, [categoryFilter, statusFilter]);

  useEffect(() => {
    load();
  }, [load]);

  const filtered = assets.filter((a) => {
    if (!search) return true;
    const q = search.toLowerCase();
    return (
      a.asset_code.toLowerCase().includes(q) ||
      a.brand.toLowerCase().includes(q) ||
      a.model.toLowerCase().includes(q)
    );
  });

  const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const safePage = Math.min(page, totalPages);

  return {
    assets,
    filtered: filtered.slice((safePage - 1) * PAGE_SIZE, safePage * PAGE_SIZE),
    isLoading,
    error,
    search,
    setSearch,
    categoryFilter,
    setCategoryFilter,
    statusFilter,
    setStatusFilter,
    page: safePage,
    totalPages,
    setPage,
    reload: load,
  };
}

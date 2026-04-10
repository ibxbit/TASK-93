import { useCallback, useEffect, useState } from "react";
import { invoicesService } from "@/services/invoices.service";
import type { Invoice, InvoiceListParams, InvoiceStatus } from "@/types";

const PAGE_SIZE = 10;

interface UseInvoicesReturn {
  invoices: Invoice[];
  filtered: Invoice[];
  isLoading: boolean;
  error: string | null;
  search: string;
  setSearch: (v: string) => void;
  statusFilter: InvoiceStatus | "";
  setStatusFilter: (v: InvoiceStatus | "") => void;
  page: number;
  totalPages: number;
  setPage: (p: number) => void;
  reload: () => void;
}

export function useInvoices(): UseInvoicesReturn {
  const [invoices, setInvoices] = useState<Invoice[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState<InvoiceStatus | "">("");
  const [page, setPage] = useState(1);

  const load = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    const params: InvoiceListParams = {};
    if (statusFilter) params.status = statusFilter;

    try {
      const data = await invoicesService.list(params);
      setInvoices(data);
      setPage(1);
    } catch (err: unknown) {
      if (err instanceof Error) {
        setError(err.message || "Failed to load invoices.");
      }
    } finally {
      setIsLoading(false);
    }
  }, [statusFilter]);

  useEffect(() => {
    load();
  }, [load]);

  const filtered = invoices.filter((inv) => {
    if (!search) return true;
    const q = search.toLowerCase();
    return (
      inv.invoice_no.toLowerCase().includes(q) ||
      inv.counterparty.toLowerCase().includes(q)
    );
  });

  const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const safePage = Math.min(page, totalPages);

  return {
    invoices,
    filtered: filtered.slice((safePage - 1) * PAGE_SIZE, safePage * PAGE_SIZE),
    isLoading,
    error,
    search,
    setSearch,
    statusFilter,
    setStatusFilter,
    page: safePage,
    totalPages,
    setPage,
    reload: load,
  };
}

import { useCallback, useEffect, useState } from "react";
import { eventsService } from "@/services/events.service";
import type { EventListParams, EventStatus, MotorsportEvent } from "@/types";

const PAGE_SIZE = 10;

interface UseEventsReturn {
  events: MotorsportEvent[];
  filtered: MotorsportEvent[];
  isLoading: boolean;
  error: string | null;
  search: string;
  setSearch: (v: string) => void;
  statusFilter: EventStatus | "";
  setStatusFilter: (v: EventStatus | "") => void;
  page: number;
  totalPages: number;
  setPage: (p: number) => void;
  reload: () => void;
}

export function useEvents(): UseEventsReturn {
  const [events, setEvents] = useState<MotorsportEvent[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState<EventStatus | "">("");
  const [page, setPage] = useState(1);

  const load = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    const params: EventListParams = {};
    if (statusFilter) params.status = statusFilter;

    try {
      const data = await eventsService.list(params);
      setEvents(data);
      setPage(1);
    } catch (err: unknown) {
      if (err instanceof Error) {
        setError(err.message || "Failed to load events.");
      }
    } finally {
      setIsLoading(false);
    }
  }, [statusFilter]);

  useEffect(() => {
    load();
  }, [load]);

  const filtered = events.filter((e) => {
    if (!search) return true;
    const q = search.toLowerCase();
    return (
      e.name.toLowerCase().includes(q) ||
      (e.venue_identifier ?? "").toLowerCase().includes(q) ||
      (e.schedule_group ?? "").toLowerCase().includes(q)
    );
  });

  const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const safePage = Math.min(page, totalPages);

  return {
    events,
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

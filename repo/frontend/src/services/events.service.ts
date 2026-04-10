import { api } from "./api";
import type { EventListParams, MotorsportEvent } from "@/types";

function buildQuery(params: EventListParams): string {
  const q = new URLSearchParams();
  if (params.status) q.set("status", params.status);
  if (params.schedule_group) q.set("schedule_group", params.schedule_group);
  const qs = q.toString();
  return qs ? `?${qs}` : "";
}

export const eventsService = {
  list(params: EventListParams = {}): Promise<MotorsportEvent[]> {
    return api.get<MotorsportEvent[]>(`/events${buildQuery(params)}`);
  },

  get(id: number): Promise<MotorsportEvent> {
    return api.get<MotorsportEvent>(`/events/${id}`);
  },
};

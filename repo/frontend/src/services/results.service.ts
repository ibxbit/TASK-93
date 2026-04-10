import { api } from "./api";
import type {
  ArbitratePayload,
  EventResult,
  RankingsResponse,
  ResultUnit,
  ReviewPayload,
} from "@/types";

export interface CreateResultPayload {
  participant_id: number;
  attempt_no: number;
  value_numeric: number;
  unit_enum: ResultUnit;
}

export const resultsService = {
  listForEvent(eventId: number): Promise<EventResult[]> {
    return api.get<EventResult[]>(`/events/${eventId}/results`);
  },

  getRankings(
    eventId: number,
    unit: ResultUnit = "points"
  ): Promise<RankingsResponse> {
    return api.get<RankingsResponse>(
      `/events/${eventId}/rankings?unit=${unit}`
    );
  },

  createResult(
    eventId: number,
    payload: CreateResultPayload
  ): Promise<EventResult> {
    return api.post<EventResult>(`/events/${eventId}/results`, payload);
  },

  submitReview(
    eventId: number,
    resultId: number,
    payload: ReviewPayload
  ): Promise<void> {
    return api.post<void>(
      `/events/${eventId}/results/${resultId}/reviews`,
      payload
    );
  },

  arbitrate(
    eventId: number,
    resultId: number,
    payload: ArbitratePayload
  ): Promise<void> {
    return api.post<void>(
      `/events/${eventId}/results/${resultId}/arbitrate`,
      payload
    );
  },
};

import { api } from "./api";
import type {
  Asset,
  AssetListParams,
  CreateAssetPayload,
  UpdateAssetStatusPayload,
} from "@/types";

function buildQuery(params: AssetListParams): string {
  const q = new URLSearchParams();
  if (params.category) q.set("category", params.category);
  if (params.status) q.set("status", params.status);
  const qs = q.toString();
  return qs ? `?${qs}` : "";
}

export const assetsService = {
  list(params: AssetListParams = {}): Promise<Asset[]> {
    return api.get<Asset[]>(`/assets${buildQuery(params)}`);
  },

  get(id: number): Promise<Asset> {
    return api.get<Asset>(`/assets/${id}`);
  },

  create(payload: CreateAssetPayload): Promise<Asset> {
    return api.post<Asset>("/assets", payload);
  },

  update(id: number, payload: Partial<CreateAssetPayload>): Promise<Asset> {
    return api.put<Asset>(`/assets/${id}`, payload);
  },

  updateStatus(id: number, payload: UpdateAssetStatusPayload): Promise<Asset> {
    return api.patch<Asset>(`/assets/${id}/status`, payload);
  },
};

import { api } from "./api";
import type {
  CreateInvoicePayload,
  Invoice,
  InvoiceListParams,
} from "@/types";

function buildQuery(params: InvoiceListParams): string {
  const q = new URLSearchParams();
  if (params.status) q.set("status", params.status);
  if (params.counterparty) q.set("counterparty", params.counterparty);
  const qs = q.toString();
  return qs ? `?${qs}` : "";
}

export const invoicesService = {
  list(params: InvoiceListParams = {}): Promise<Invoice[]> {
    return api.get<Invoice[]>(`/invoices${buildQuery(params)}`);
  },

  get(id: number): Promise<Invoice> {
    return api.get<Invoice>(`/invoices/${id}`);
  },

  create(payload: CreateInvoicePayload): Promise<Invoice> {
    return api.post<Invoice>("/invoices", payload);
  },

  issue(id: number): Promise<Invoice> {
    return api.post<Invoice>(`/invoices/${id}/issue`, {});
  },
};

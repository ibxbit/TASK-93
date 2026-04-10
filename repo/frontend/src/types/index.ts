// ── Auth ──────────────────────────────────────────────────────────────────────
export interface LoginRequest {
  username: string;
  password: string;
}

export interface LoginResponse {
  token: string;
  expires_at: string; // ISO 8601
}

export type Role =
  | "Administrator"
  | "EventDirector"
  | "Referee"
  | "FinanceClerk"
  | "Auditor";

export interface AuthUser {
  user_id: number;
  username: string;
  roles: Role[];
  token: string;
}

// ── Assets ────────────────────────────────────────────────────────────────────
export type AssetCategory =
  | "vehicle"
  | "equipment"
  | "facility"
  | "electronic"
  | "other";

export type AssetStatus = "in_service" | "out_for_repair" | "retired";

export interface Asset {
  id: number;
  asset_code: string;
  category: AssetCategory;
  brand: string;
  model: string;
  status: AssetStatus;
  procurement_cost: string | null;
  procurement_date: string | null;
  useful_life_months: number | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface AssetListParams {
  category?: AssetCategory;
  status?: AssetStatus;
  page?: number;
  page_size?: number;
}

// ── Events ────────────────────────────────────────────────────────────────────
export type EventStatus =
  | "draft"
  | "published"
  | "in_progress"
  | "completed"
  | "cancelled";

export interface MotorsportEvent {
  id: number;
  name: string;
  description: string | null;
  venue_identifier: string | null;
  schedule_group: string | null;
  status: EventStatus;
  is_championship_class: boolean;
  created_at: string;
  updated_at: string;
}

// ── Results ───────────────────────────────────────────────────────────────────
export type ResultUnit =
  | "milliseconds"
  | "seconds"
  | "meters"
  | "kilometers"
  | "feet"
  | "inches"
  | "kilograms"
  | "points";

export type ReviewedState = "pending" | "approved" | "rejected";

export interface EventResult {
  id: number;
  event_id: number;
  participant_id: number;
  attempt_no: number;
  value_numeric: number;
  unit_enum: ResultUnit;
  reviewed_state: ReviewedState;
  entered_by: number;
  created_at: string;
  updated_at: string;
}

export interface RankingEntry {
  rank: number;
  participant_id: number;
  value_numeric: number;
  unit_enum: ResultUnit;
}

export interface RankingsResponse {
  event_id: number;
  unit: ResultUnit;
  rankings: RankingEntry[];
}

export interface EventListParams {
  status?: EventStatus;
  schedule_group?: string;
}

// ── Invoices ──────────────────────────────────────────────────────────────────
export type InvoiceStatus =
  | "draft"
  | "issued"
  | "paid"
  | "cancelled"
  | "overdue";

export interface Invoice {
  id: number;
  invoice_no: string;          // e.g. "INV-2024-0042"
  counterparty: string;
  issue_date: string;          // YYYY-MM-DD
  tax_rate: string;            // Decimal as string, e.g. "0.1000"
  subtotal: string;
  tax: string;
  discount_amount: string;
  total: string;
  status: InvoiceStatus;
  created_by: number;
  created_at: string;
  updated_at: string;
}

export interface InvoiceListParams {
  status?: InvoiceStatus;
  counterparty?: string;
}

export interface CreateInvoicePayload {
  counterparty: string;
  issue_date: string;  // YYYY-MM-DD
  tax_rate?: string;   // e.g. "0.10"
}

// ── Asset Mutations ───────────────────────────────────────────────────────────
export interface CreateAssetPayload {
  asset_code: string;
  category: AssetCategory;
  brand: string;
  model: string;
  notes?: string;
  procurement_cost?: string;
  procurement_date?: string;    // YYYY-MM-DD
  useful_life_months?: number;
}

export interface UpdateAssetStatusPayload {
  status: AssetStatus;
}

// ── Result Mutations ──────────────────────────────────────────────────────────
export interface ArbitratePayload {
  decision: "approved" | "rejected";
  reason?: string;
}

export interface ReviewPayload {
  decision: "approved" | "rejected";
  comment?: string;
}

// ── API Error ─────────────────────────────────────────────────────────────────
export interface ApiError {
  code: string;
  message: string;
  correlation_id?: string;
}

// ── Pagination ────────────────────────────────────────────────────────────────
export interface PaginatedResult<T> {
  data: T[];
  total: number;
  page: number;
  page_size: number;
}

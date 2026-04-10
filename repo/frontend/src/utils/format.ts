export function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString("en-AU", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function formatCurrency(value: string | null): string {
  if (!value) return "—";
  const num = parseFloat(value);
  return isNaN(num)
    ? "—"
    : new Intl.NumberFormat("en-AU", {
        style: "currency",
        currency: "AUD",
      }).format(num);
}

export function labelAssetCategory(cat: string): string {
  const map: Record<string, string> = {
    vehicle: "Vehicle",
    equipment: "Equipment",
    facility: "Facility",
    electronic: "Electronic",
    other: "Other",
  };
  return map[cat] ?? cat;
}

export function labelAssetStatus(s: string): string {
  const map: Record<string, string> = {
    in_service: "In Service",
    out_for_repair: "Out for Repair",
    retired: "Retired",
  };
  return map[s] ?? s;
}

export function labelRole(r: string): string {
  const map: Record<string, string> = {
    Administrator: "Administrator",
    EventDirector: "Event Director",
    Referee: "Referee",
    FinanceClerk: "Finance Clerk",
    Auditor: "Auditor",
  };
  return map[r] ?? r;
}

export function labelEventStatus(s: string): string {
  const map: Record<string, string> = {
    draft: "Draft",
    published: "Published",
    in_progress: "In Progress",
    completed: "Completed",
    cancelled: "Cancelled",
  };
  return map[s] ?? s;
}

export function labelReviewedState(s: string): string {
  const map: Record<string, string> = {
    pending: "Pending",
    approved: "Approved",
    rejected: "Rejected",
  };
  return map[s] ?? s;
}

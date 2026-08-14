export type Where = {
  section: "queue" | "history" | "insights" | "settings" | null;
  owner: string | null;
  name: string | null;
  branch: string | null;
};

const nowhere: Where = { section: null, owner: null, name: null, branch: null };

export function whereWeAre(pathname: string): Where {
  const [section, owner, name, branch] = pathname.split("/").filter(Boolean);
  if (!owner || !name) return nowhere;
  if (section === "settings") return { section, owner, name, branch: null };
  if (section === "queue" || section === "history" || section === "insights") {
    return branch ? { section, owner, name, branch } : nowhere;
  }
  return nowhere;
}

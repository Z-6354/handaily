import { createContext, useContext, type ReactNode } from "react";
import { useWikiBulkImport } from "../hooks/useWikiBulkImport";

export type WikiBulkImportContextValue = ReturnType<typeof useWikiBulkImport>;

const WikiBulkImportContext = createContext<WikiBulkImportContextValue | null>(null);

export function WikiBulkImportProvider({ children }: { children: ReactNode }) {
  const bulk = useWikiBulkImport();
  return (
    <WikiBulkImportContext.Provider value={bulk}>{children}</WikiBulkImportContext.Provider>
  );
}

export function useWikiBulkImportContext(): WikiBulkImportContextValue {
  const ctx = useContext(WikiBulkImportContext);
  if (!ctx) {
    throw new Error("useWikiBulkImportContext must be used within WikiBulkImportProvider");
  }
  return ctx;
}

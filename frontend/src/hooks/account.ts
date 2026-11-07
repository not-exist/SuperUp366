import { createContext, useContext } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../api/commands";
import type { Account } from "../api/types";

/** The active account, provided by the shell so pages do not re-query it. */
export const AccountContext = createContext<Account | null>(null);

export function useAccount(): Account {
  const account = useContext(AccountContext);
  if (!account) {
    throw new Error("AccountContext is missing");
  }
  return account;
}

export function useAccounts() {
  return useQuery({
    queryKey: ["accounts"],
    queryFn: api.listAccounts,
    staleTime: 10_000,
  });
}

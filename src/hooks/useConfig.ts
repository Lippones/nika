import { useCallback, useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { Config } from "../lib/types";

interface ConfigState {
  config: Config | null;
  save: (next: Config) => Promise<void>;
  saving: boolean;
  error: string | null;
}

export function useConfig(): ConfigState {
  const [config, setConfig] = useState<Config | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;
    api
      .getConfig()
      .then((loaded) => mounted && setConfig(loaded))
      .catch((failure) => mounted && setError(errorMessage(failure)));

    return () => {
      mounted = false;
    };
  }, []);

  const save = useCallback(async (next: Config) => {
    setSaving(true);
    setError(null);
    try {
      // O core devolve a config efetivamente gravada — é ela que vale.
      setConfig(await api.setConfig(next));
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setSaving(false);
    }
  }, []);

  return { config, save, saving, error };
}

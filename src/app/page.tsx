"use client";

import { Suspense, useCallback, useEffect, useMemo, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type JobSettings = {
  preferredTitles: string[];
  locations: string[];
  keywords: string[];
  remoteOnly: boolean;
  salaryMin: number | null;
  salaryMax: number | null;
  companyBlacklist: string[];
};

type JobMatch = {
  id: string;
  analysis_id?: string | null;
  url: string;
  title?: string | null;
  company?: string | null;
  location?: string | null;
  match_score: number;
  summary: string;
  created_at: string;
  raw_excerpt?: string | null;
};

const defaultSettings: JobSettings = {
  preferredTitles: [
    "Software Engineer",
    "Full Stack Engineer",
    "Frontend Engineer",
    "Backend Engineer",
  ],
  locations: ["Remote", "United States"],
  keywords: ["TypeScript", "React", "Node.js", "Rust", "Tauri", "Next.js"],
  remoteOnly: true,
  salaryMin: 120000,
  salaryMax: 200000,
  companyBlacklist: [],
};

const parseList = (value: string) =>
  value
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter(Boolean);

const listToText = (value: string[]) => value.join("\n");

export default function Home() {
  return (
    <Suspense fallback={<div className="p-10 text-sm">Loading...</div>}>
      <HomeContent />
    </Suspense>
  );
}

function HomeContent() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const [jobUrl, setJobUrl] = useState("");
  const [analysisId, setAnalysisId] = useState<string | null>(null);
  const [status, setStatus] = useState<"idle" | "running" | "done" | "error">(
    "idle"
  );
  const [statusMessage, setStatusMessage] = useState("");
  const [settings, setSettings] = useState<JobSettings>(defaultSettings);
  const [matches, setMatches] = useState<JobMatch[]>([]);
  const [tauriReady, setTauriReady] = useState(false);

  useEffect(() => {
    const paramUrl = searchParams.get("url");
    if (paramUrl && paramUrl !== jobUrl) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setJobUrl(paramUrl);
    }
  }, [searchParams, jobUrl]);

  const refreshMatches = useCallback(() => {
    if (!tauriReady) return;
    invoke<JobMatch[]>("list_job_matches", { limit: 50 })
      .then((data) => setMatches(data))
      .catch(() => setMatches([]));
  }, [tauriReady]);

  useEffect(() => {
    const ready = isTauri();
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setTauriReady(ready);
    if (!ready) {
      return;
    }

    invoke<JobSettings>("get_settings")
      .then((data) => setSettings(data))
      .catch(() => setSettings(defaultSettings));

    refreshMatches();
  }, [refreshMatches]);

  useEffect(() => {
    if (!tauriReady) return;
    let unlistenQuery: (() => void) | undefined;
    let unlistenReload: (() => void) | undefined;
    let unlistenStarted: (() => void) | undefined;
    let unlistenCompleted: (() => void) | undefined;

    const setup = async () => {
      unlistenQuery = await listen("mcp:set-query-params", (event) => {
        const payload = event.payload as { url?: string; analysisId?: string };
        if (payload?.url) {
          const params = new URLSearchParams(searchParams.toString());
          params.set("url", payload.url);
          if (payload.analysisId) {
            params.set("analysisId", payload.analysisId);
            setAnalysisId(payload.analysisId);
          }
          router.replace(`/?${params.toString()}`);
        }
      });

      unlistenReload = await listen("mcp:reload", () => {
        window.location.reload();
      });

      unlistenStarted = await listen("analysis:started", (event) => {
        const payload = event.payload as { analysisId?: string };
        setStatus("running");
        setStatusMessage("Agents are dissecting the listing...");
        if (payload?.analysisId) {
          setAnalysisId(payload.analysisId);
        }
      });

      unlistenCompleted = await listen("analysis:completed", () => {
        setStatus("done");
        setStatusMessage("Analysis complete. Match saved.");
        refreshMatches();
      });
    };

    setup();
    return () => {
      unlistenQuery?.();
      unlistenReload?.();
      unlistenStarted?.();
      unlistenCompleted?.();
    };
  }, [tauriReady, router, searchParams, refreshMatches]);

  const handleAnalyze = async () => {
    if (!jobUrl.trim()) {
      setStatus("error");
      setStatusMessage("Add a job listing URL to begin.");
      return;
    }
    const params = new URLSearchParams(searchParams.toString());
    params.set("url", jobUrl.trim());
    router.replace(`/?${params.toString()}`);

    if (!tauriReady) {
      setStatus("error");
      setStatusMessage("Tauri backend not available in browser preview.");
      return;
    }
    setStatus("running");
    setStatusMessage("Dispatching the agent swarm...");
    try {
      const response = await invoke<{ analysisId: string }>("start_analysis", {
        url: jobUrl.trim(),
      });
      setAnalysisId(response.analysisId);
    } catch {
      setStatus("error");
      setStatusMessage("Failed to start analysis agent.");
    }
  };

  const handleSaveSettings = async () => {
    if (!tauriReady) return;
    const updated = await invoke<JobSettings>("update_settings", {
      settings,
    });
    setSettings(updated);
    setStatusMessage("Settings saved.");
  };

  const headerStatus = useMemo(() => {
    switch (status) {
      case "running":
        return "Running";
      case "done":
        return "Ready";
      case "error":
        return "Needs attention";
      default:
        return "Idle";
    }
  }, [status]);

  return (
    <div className="min-h-screen px-6 py-10 text-sm md:px-14">
      <div className="mx-auto flex max-w-6xl flex-col gap-8">
        <header className="flex flex-col gap-6">
          <div className="flex flex-wrap items-center justify-between gap-6">
            <div className="flex flex-col gap-2">
              <p className="text-xs font-semibold uppercase tracking-[0.3em] text-[#0f766e]">
                Job Hunter Console
              </p>
              <h1 className="text-4xl font-semibold text-[#1c1a17] md:text-5xl">
                Agent-led job analysis for your next move.
              </h1>
              <p className="max-w-2xl text-base text-[#3e352b]">
                Feed a job listing URL. The multi-agent pipeline extracts role
                data, scores fit against your preferences, and saves matches
                into SQLite for review.
              </p>
            </div>
            <div className="app-shell accent-ring rounded-2xl px-6 py-4 text-sm">
              <p className="text-xs uppercase tracking-[0.2em] text-[#1e3a8a]">
                Status
              </p>
              <p className="mt-2 text-2xl font-semibold text-[#1c1a17]">
                {headerStatus}
              </p>
              <p className="text-xs text-[#6a5c4d]">{statusMessage}</p>
            </div>
          </div>
        </header>

        <section className="grid gap-6 lg:grid-cols-[1.3fr_0.7fr]">
          <div className="app-shell rounded-3xl p-8">
            <div className="flex flex-col gap-6">
              <div>
                <p className="text-xs font-semibold uppercase tracking-[0.2em] text-[#1e3a8a]">
                  Listing Input
                </p>
                <h2 className="mt-2 text-2xl font-semibold text-[#1c1a17]">
                  Analyze a new job post
                </h2>
                <p className="text-sm text-[#5b4d3f]">
                  URL parameters are kept in sync so you can share a deep link
                  back to this analysis session.
                </p>
              </div>
              <div className="flex flex-col gap-4">
                <label className="text-xs font-semibold uppercase tracking-[0.2em] text-[#1c1a17]">
                  Job URL
                </label>
                <input
                  className="w-full rounded-2xl border border-[#e0d5c4] bg-white/80 px-4 py-3 text-base text-[#1c1a17] shadow-sm focus:border-[#f97316] focus:outline-none"
                  placeholder="https://careers.company.com/jobs/123"
                  value={jobUrl}
                  onChange={(event) => setJobUrl(event.target.value)}
                />
                <div className="flex flex-wrap items-center gap-4">
                  <button
                    onClick={handleAnalyze}
                    className="rounded-full bg-[#f97316] px-6 py-3 text-sm font-semibold text-white transition hover:bg-[#ea580c]"
                  >
                    Launch Analysis
                  </button>
                  <div className="text-xs text-[#6a5c4d]">
                    Analysis ID: {analysisId ?? "Not started"}
                  </div>
                </div>
                {!tauriReady && (
                  <p className="rounded-xl bg-[#fef3c7] px-4 py-2 text-xs text-[#92400e]">
                    Tauri backend not detected. Launch via `npm run tauri dev`
                    for full functionality.
                  </p>
                )}
              </div>
            </div>
          </div>

          <div className="app-shell rounded-3xl p-6">
            <p className="text-xs font-semibold uppercase tracking-[0.2em] text-[#1e3a8a]">
              Multi-agent flow
            </p>
            <h3 className="mt-2 text-xl font-semibold text-[#1c1a17]">
              Two-stage reasoning
            </h3>
            <div className="mt-6 flex flex-col gap-4 text-sm text-[#5b4d3f]">
              <div className="rounded-2xl border border-[#f2e6d3] bg-white/70 p-4">
                <p className="text-xs font-semibold uppercase tracking-[0.2em] text-[#0f766e]">
                  Extractor Agent
                </p>
                <p className="mt-2">
                  Pulls structured fields from the listing and normalizes
                  location, company, and role signals.
                </p>
              </div>
              <div className="rounded-2xl border border-[#f2e6d3] bg-white/70 p-4">
                <p className="text-xs font-semibold uppercase tracking-[0.2em] text-[#0f766e]">
                  Matcher Agent
                </p>
                <p className="mt-2">
                  Scores fit using your preferences and writes the match into
                  SQLite for recall.
                </p>
              </div>
            </div>
          </div>
        </section>

        <section className="grid gap-6 lg:grid-cols-[0.8fr_1.2fr]">
          <div className="app-shell rounded-3xl p-6">
            <p className="text-xs font-semibold uppercase tracking-[0.2em] text-[#1e3a8a]">
              Preferences
            </p>
            <h3 className="mt-2 text-xl font-semibold text-[#1c1a17]">
              Personal job search settings
            </h3>
            <div className="mt-6 flex flex-col gap-4 text-sm">
              <div>
                <label className="text-xs font-semibold uppercase tracking-[0.2em] text-[#1c1a17]">
                  Preferred titles
                </label>
                <textarea
                  className="mt-2 w-full rounded-2xl border border-[#e0d5c4] bg-white/80 px-3 py-2 text-sm"
                  rows={3}
                  value={listToText(settings.preferredTitles)}
                  onChange={(event) =>
                    setSettings((prev) => ({
                      ...prev,
                      preferredTitles: parseList(event.target.value),
                    }))
                  }
                />
              </div>
              <div>
                <label className="text-xs font-semibold uppercase tracking-[0.2em] text-[#1c1a17]">
                  Target locations
                </label>
                <textarea
                  className="mt-2 w-full rounded-2xl border border-[#e0d5c4] bg-white/80 px-3 py-2 text-sm"
                  rows={2}
                  value={listToText(settings.locations)}
                  onChange={(event) =>
                    setSettings((prev) => ({
                      ...prev,
                      locations: parseList(event.target.value),
                    }))
                  }
                />
              </div>
              <div>
                <label className="text-xs font-semibold uppercase tracking-[0.2em] text-[#1c1a17]">
                  Keywords
                </label>
                <textarea
                  className="mt-2 w-full rounded-2xl border border-[#e0d5c4] bg-white/80 px-3 py-2 text-sm"
                  rows={3}
                  value={listToText(settings.keywords)}
                  onChange={(event) =>
                    setSettings((prev) => ({
                      ...prev,
                      keywords: parseList(event.target.value),
                    }))
                  }
                />
              </div>
              <div className="flex items-center justify-between rounded-2xl border border-[#e0d5c4] bg-white/70 px-4 py-3">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-[0.2em] text-[#1c1a17]">
                    Remote only
                  </p>
                  <p className="text-xs text-[#6a5c4d]">
                    Prioritize listings that explicitly mention remote work.
                  </p>
                </div>
                <input
                  type="checkbox"
                  checked={settings.remoteOnly}
                  onChange={(event) =>
                    setSettings((prev) => ({
                      ...prev,
                      remoteOnly: event.target.checked,
                    }))
                  }
                  className="h-5 w-5 accent-[#f97316]"
                />
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                <div>
                  <label className="text-xs font-semibold uppercase tracking-[0.2em] text-[#1c1a17]">
                    Salary min
                  </label>
                  <input
                    className="mt-2 w-full rounded-2xl border border-[#e0d5c4] bg-white/80 px-3 py-2 text-sm"
                    type="number"
                    value={settings.salaryMin ?? ""}
                    onChange={(event) =>
                      setSettings((prev) => ({
                        ...prev,
                        salaryMin: event.target.value
                          ? Number(event.target.value)
                          : null,
                      }))
                    }
                  />
                </div>
                <div>
                  <label className="text-xs font-semibold uppercase tracking-[0.2em] text-[#1c1a17]">
                    Salary max
                  </label>
                  <input
                    className="mt-2 w-full rounded-2xl border border-[#e0d5c4] bg-white/80 px-3 py-2 text-sm"
                    type="number"
                    value={settings.salaryMax ?? ""}
                    onChange={(event) =>
                      setSettings((prev) => ({
                        ...prev,
                        salaryMax: event.target.value
                          ? Number(event.target.value)
                          : null,
                      }))
                    }
                  />
                </div>
              </div>
              <div>
                <label className="text-xs font-semibold uppercase tracking-[0.2em] text-[#1c1a17]">
                  Company blacklist
                </label>
                <textarea
                  className="mt-2 w-full rounded-2xl border border-[#e0d5c4] bg-white/80 px-3 py-2 text-sm"
                  rows={2}
                  value={listToText(settings.companyBlacklist)}
                  onChange={(event) =>
                    setSettings((prev) => ({
                      ...prev,
                      companyBlacklist: parseList(event.target.value),
                    }))
                  }
                />
              </div>
              <button
                onClick={handleSaveSettings}
                className="rounded-full border border-[#1e3a8a] px-5 py-2 text-xs font-semibold uppercase tracking-[0.2em] text-[#1e3a8a] transition hover:bg-[#1e3a8a] hover:text-white"
              >
                Save settings
              </button>
            </div>
          </div>

          <div className="app-shell rounded-3xl p-6">
            <p className="text-xs font-semibold uppercase tracking-[0.2em] text-[#1e3a8a]">
              Matches
            </p>
            <h3 className="mt-2 text-xl font-semibold text-[#1c1a17]">
              Stored job fits
            </h3>
            <div className="mt-6 flex flex-col gap-4">
              {matches.length === 0 ? (
                <div className="rounded-2xl border border-dashed border-[#e0d5c4] px-4 py-6 text-sm text-[#6a5c4d]">
                  No matches saved yet. Run an analysis to populate SQLite.
                </div>
              ) : (
                matches.map((match) => (
                  <div
                    key={match.id}
                    className="rounded-2xl border border-[#f2e6d3] bg-white/70 p-4"
                  >
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div>
                        <p className="text-xs uppercase tracking-[0.2em] text-[#0f766e]">
                          Match score {Math.round(match.match_score)}%
                        </p>
                        <p className="text-lg font-semibold text-[#1c1a17]">
                          {match.title ?? "Untitled role"}
                        </p>
                        <p className="text-sm text-[#5b4d3f]">
                          {match.company ?? "Unknown company"} ·{" "}
                          {match.location ?? "Location not detected"}
                        </p>
                      </div>
                      <a
                        className="rounded-full border border-[#f97316] px-3 py-1 text-xs font-semibold text-[#f97316] hover:bg-[#f97316] hover:text-white"
                        href={match.url}
                        target="_blank"
                        rel="noreferrer"
                      >
                        Open listing
                      </a>
                    </div>
                    <p className="mt-3 text-sm text-[#6a5c4d]">
                      {match.summary}
                    </p>
                    {match.raw_excerpt && (
                      <p className="mt-3 text-xs text-[#8a7a69]">
                        {match.raw_excerpt}
                      </p>
                    )}
                    <p className="mt-3 text-xs text-[#a08c7a]">
                      Saved {new Date(match.created_at).toLocaleString()}
                    </p>
                  </div>
                ))
              )}
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

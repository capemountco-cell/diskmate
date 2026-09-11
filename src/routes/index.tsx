import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/")({
  component: Index,
});

function Index() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center bg-background text-foreground">
      <h1 className="text-4xl font-semibold tracking-tight">DiskMate</h1>
      <p className="mt-2 text-sm text-muted-foreground">Storage, understood.</p>
    </main>
  );
}

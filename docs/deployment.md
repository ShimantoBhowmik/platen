# Deployment

platen runs as one container: the server plus the frontend bundle it serves at `/`. Render
hosts the container on its free plan. Postgres is hosted on Neon, also on its free plan. A
push to `main` deploys after CI passes. Migrations run when the server starts.

| File | Use |
|---|---|
| `Dockerfile` | Builds the production image: the frontend bundle and the server binary. |
| `.dockerignore` | Keeps local builds, secrets and tooling out of the image build. |
| `render.yaml` | The Render Blueprint: one web service built from the `Dockerfile`. |

These files stay at the repository root. Render reads `render.yaml` from the root, and Docker
reads `.dockerignore` from the root of the build context.

## First deploy

1. In Neon, create a project with Postgres 17 in AWS US East 1 (N. Virginia). Copy the direct
   connection string, the one without `-pooler` in the host.
2. In Render, select **New > Blueprint** and select this repository.
3. When Render asks for `DATABASE_URL`, paste the Neon string. Render generates
   `TOKEN_SECRET`.

## Build the image locally

From the repository root:

```bash
docker build -t platen .
```

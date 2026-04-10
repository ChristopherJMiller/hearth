import {
  createRouter,
  createRoute,
  createRootRoute,
  redirect,
} from '@tanstack/react-router';
import { RootLayout } from './routes/__root';
import { DashboardPage } from './routes/dashboard';
import { MachinesPage } from './routes/machines/index';
import { MachineDetailPage } from './routes/machines/$machineId';
import { EnrollmentPage } from './routes/enrollment/index';
import { DeploymentsPage } from './routes/deployments/index';
import { DeploymentDetailPage } from './routes/deployments/$deploymentId';
import { NewDeploymentPage } from './routes/deployments/new';
import { CatalogManagePage } from './routes/catalog/manage';
import { CatalogBrowsePage } from './routes/catalog/browse';
import { RequestsPage } from './routes/requests/index';
import { AuditPage } from './routes/audit/index';
import { ReportsPage } from './routes/reports';
import { CompliancePage } from './routes/compliance';
import { ServicesPage } from './routes/services';
import { DirectoryPage } from './routes/directory';
import { SettingsPage } from './routes/settings';
import { BuildsPage } from './routes/builds/index';
import { BuildJobDetailPage } from './routes/builds/$jobId';
import { PeoplePage } from './routes/people/index';
import { PersonDetailPage } from './routes/people/$username';
import { MyEnvironmentPage } from './routes/me/environment';
import { HealthPage } from './routes/health';
import { MeshPage } from './routes/mesh';

const rootRoute = createRootRoute({
  component: RootLayout,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  beforeLoad: () => {
    throw redirect({ to: '/dashboard' });
  },
});

// ── Fleet ───────────────────────────────────────────────────────────────────
const dashboardRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/dashboard',
  component: DashboardPage,
});

const machinesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/machines',
  component: MachinesPage,
});

const machineDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/machines/$machineId',
  component: MachineDetailPage,
});

const enrollmentRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/enrollment',
  component: EnrollmentPage,
});

const meshRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/mesh',
  component: MeshPage,
});

// ── Software ────────────────────────────────────────────────────────────────
const deploymentsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/deployments',
  component: DeploymentsPage,
});

const deploymentDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/deployments/$deploymentId',
  component: DeploymentDetailPage,
});

const newDeploymentRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/deployments/new',
  component: NewDeploymentPage,
});

const buildsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/builds',
  component: BuildsPage,
});

const buildDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/builds/$jobId',
  component: BuildJobDetailPage,
});

const catalogBrowseRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/catalog',
  component: CatalogBrowsePage,
});

const catalogManageRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/catalog/manage',
  component: CatalogManagePage,
});

const requestsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/requests',
  component: RequestsPage,
});

// ── Identity & access ───────────────────────────────────────────────────────
const peopleRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/people',
  component: PeoplePage,
});

const personDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/people/$username',
  component: PersonDetailPage,
});

const directoryRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/directory',
  component: DirectoryPage,
});

const auditRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/audit',
  component: AuditPage,
});

// ── Observability ───────────────────────────────────────────────────────────
const healthRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/health',
  component: HealthPage,
});

const complianceRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/compliance',
  component: CompliancePage,
});

const reportsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/reports',
  component: ReportsPage,
});

const servicesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/services',
  component: ServicesPage,
});

// ── Personal ────────────────────────────────────────────────────────────────
const myEnvironmentRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/me/environment',
  component: MyEnvironmentPage,
});

const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/settings',
  component: SettingsPage,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  dashboardRoute,
  machinesRoute,
  machineDetailRoute,
  enrollmentRoute,
  meshRoute,
  deploymentsRoute,
  newDeploymentRoute,
  deploymentDetailRoute,
  buildsRoute,
  buildDetailRoute,
  catalogBrowseRoute,
  catalogManageRoute,
  requestsRoute,
  peopleRoute,
  personDetailRoute,
  directoryRoute,
  auditRoute,
  healthRoute,
  complianceRoute,
  reportsRoute,
  servicesRoute,
  myEnvironmentRoute,
  settingsRoute,
]);

export const router = createRouter({
  routeTree,
  basepath: '/',
});

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}

/* ============================================================================
   App —— 一级导航路由（10 个业务页），全部在 Soft bloom 工作区内渲染。
   ============================================================================ */
import { AppShell } from "./components/Shell";
import { useApp } from "./runtime/AppContext";

import OperationsDashboardSection from "./components/sections/OperationsDashboardSection";
import ProductManagementSection from "./components/sections/ProductManagementSection";
import OrderManagementSection from "./components/sections/OrderManagementSection";
import PublishWorkbenchSection from "./components/sections/PublishWorkbenchSection";
import ProcurementSection from "./components/sections/ProcurementSection";
import PriceUpdateSection from "./components/sections/PriceUpdateSection";
import OperationsExceptionsSection from "./components/sections/OperationsExceptionsSection";
import OperationsAnalyticsSection from "./components/sections/OperationsAnalyticsSection";
import AgentSkillsSection from "./components/sections/AgentSkillsSection";
import SystemSettingsSection from "./components/sections/SystemSettingsSection";

export default function App() {
  const ctx = useApp();
  const section = ctx.selectedSection.value;
  return (
    <AppShell>
      {section === "workbench" && <OperationsDashboardSection />}
      {section === "products" && <ProductManagementSection />}
      {section === "orders" && <OrderManagementSection />}
      {section === "publish" && <PublishWorkbenchSection />}
      {section === "procurement" && <ProcurementSection />}
      {section === "price" && <PriceUpdateSection />}
      {section === "exceptions" && <OperationsExceptionsSection />}
      {section === "analytics" && <OperationsAnalyticsSection />}
      {section === "skills" && <AgentSkillsSection />}
      {section === "settings" && <SystemSettingsSection />}
    </AppShell>
  );
}

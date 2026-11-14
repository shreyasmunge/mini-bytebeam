import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { Layout } from "./components/Layout";
import { MapPage } from "./pages/MapPage";
import { AnalyticsPage } from "./pages/AnalyticsPage";
import { ThresholdsPage } from "./pages/ThresholdsPage";
import { AlertsPage } from "./pages/AlertsPage";
import { MetricsPage } from "./pages/MetricsPage";

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<MapPage />} />
          <Route path="analytics" element={<AnalyticsPage />} />
          <Route path="alerts" element={<AlertsPage />} />
          <Route path="pipeline" element={<MetricsPage />} />
          <Route path="thresholds" element={<ThresholdsPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}

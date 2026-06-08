import { createRoot } from "react-dom/client";
import App from "./App";
import { AppProvider } from "./runtime/AppContext";
import { FeedbackHost } from "./runtime/feedback";
import "./styles/app.css";

createRoot(document.getElementById("root")!).render(
  <AppProvider>
    <App />
    <FeedbackHost />
  </AppProvider>,
);

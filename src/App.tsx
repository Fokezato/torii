import { createHashRouter, RouterProvider } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AppShell } from "@/components/layout/AppShell";
import Home from "@/routes/Home";
import Library from "@/routes/Library";
import LibraryDetail from "@/routes/LibraryDetail";
import Downloads from "@/routes/Downloads";
import Config from "@/routes/Config";
import NotificationWindow from "@/routes/NotificationWindow";
import PlayerOverlay from "@/routes/PlayerOverlay";
import Player from "@/routes/Player";

const queryClient = new QueryClient();

const router = createHashRouter([
  {
    element: <AppShell />,
    children: [
      { path: "/", element: <Home /> },
      { path: "/library", element: <Library /> },
      { path: "/library/:id", element: <LibraryDetail /> },
      { path: "/downloads", element: <Downloads /> },
      { path: "/config", element: <Config /> },
    ],
  },
  { path: "/notification-window", element: <NotificationWindow /> },
  { path: "/player-overlay", element: <PlayerOverlay /> },
  // Fora do AppShell de propósito: player ocupa a janela inteira.
  { path: "/watch/:watchId/:episode", element: <Player /> },
]);

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}

export default App;

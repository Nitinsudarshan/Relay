import { LoginForm } from "@/components/login-form";
import { Toaster } from "@/components/ui/sonner";

export default function LoginPage() {
  return (
    <main className="min-h-screen w-full bg-background text-foreground flex flex-col items-center justify-center p-4 relative overflow-hidden font-sans">
      <Toaster />

      {/* Subtle Background Accent Glows */}
      <div className="absolute inset-0 pointer-events-none z-0">
        <div className="absolute top-[-20%] left-[-10%] w-[60%] h-[60%] bg-primary/10 rounded-full blur-[140px]" />
        <div className="absolute bottom-[-20%] right-[-10%] w-[60%] h-[60%] bg-accent/20 rounded-full blur-[140px]" />
      </div>

      <div className="w-full flex items-center justify-center z-10">
        <LoginForm />
      </div>
    </main>
  );
}

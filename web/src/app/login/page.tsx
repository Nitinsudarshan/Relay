import { LoginForm } from "@/components/login-form"
import { Toaster } from "@/components/ui/sonner"

export default function LoginPage() {
  return (
    <div className="min-h-screen w-full bg-slate-50 dark:bg-zinc-950 flex flex-col items-center relative overflow-hidden font-sans selection:bg-primary/20 selection:text-primary">
      <Toaster />

      {/* Background Gradient */}
      <div className="absolute inset-0 pointer-events-none z-0">
        <div className="absolute top-[-20%] left-[-10%] w-[80%] h-[80%] bg-gradient-to-br from-slate-200/40 dark:from-primary/10 to-transparent rounded-full blur-[120px]"></div>
        <div className="absolute bottom-[-20%] right-[-10%] w-[80%] h-[80%] bg-gradient-to-tl from-indigo-200/40 dark:from-primary/10 to-transparent rounded-full blur-[120px]"></div>
      </div>

      <div className="flex-1 w-full flex items-center justify-center p-4 z-10">
        <LoginForm />
      </div>
    </div>
  )
}

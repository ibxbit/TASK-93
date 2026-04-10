import { useEffect } from "react";
import { useRouter } from "next/router";
import Head from "next/head";
import { useAuth } from "@/context/AuthContext";
import LoginForm from "@/components/auth/LoginForm";
import ToastContainer from "@/components/ui/ToastContainer";
import styles from "./login.module.css";

export default function LoginPage() {
  const { user, isLoading } = useAuth();
  const router = useRouter();

  useEffect(() => {
    if (!isLoading && user) {
      router.replace("/dashboard");
    }
  }, [user, isLoading, router]);

  if (isLoading) return null;

  return (
    <>
      <Head>
        <title>Sign In — Motorsport Ops</title>
      </Head>
      <div className={styles.page}>
        <div className={styles.card}>
          <LoginForm onSuccess={() => router.push("/dashboard")} />
        </div>
      </div>
      <ToastContainer />
    </>
  );
}

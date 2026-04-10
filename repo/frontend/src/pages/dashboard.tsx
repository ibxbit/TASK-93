import { useEffect } from "react";
import { useRouter } from "next/router";
import Head from "next/head";
import { useAuth } from "@/context/AuthContext";
import Layout from "@/components/layout/Layout";
import RoleNav from "@/components/dashboard/RoleNav";
import StatCard from "@/components/dashboard/StatCard";
import Spinner from "@/components/ui/Spinner";
import styles from "./dashboard.module.css";

export default function DashboardPage() {
  const { user, isLoading } = useAuth();
  const router = useRouter();

  useEffect(() => {
    if (!isLoading && !user) {
      router.replace("/login");
    }
  }, [user, isLoading, router]);

  if (isLoading) return <Spinner label="Loading dashboard…" />;
  if (!user) return null;

  return (
    <>
      <Head>
        <title>Dashboard — Motorsport Ops</title>
      </Head>
      <Layout>
        <div className={styles.header}>
          <div>
            <h1 className={styles.heading}>Dashboard</h1>
            <p className={styles.sub}>
              Welcome back, <strong>{user.username}</strong>
            </p>
          </div>
        </div>

        <div className={styles.statsGrid}>
          <StatCard icon="🏎" label="Active Events" value="—" accent="blue" />
          <StatCard icon="🔧" label="Assets In Service" value="—" accent="green" />
          <StatCard icon="💰" label="Open Invoices" value="—" accent="amber" />
          <StatCard icon="📋" label="Pending Reviews" value="—" accent="red" />
        </div>

        <div className={styles.section}>
          <RoleNav userRoles={user.roles} />
        </div>
      </Layout>
    </>
  );
}

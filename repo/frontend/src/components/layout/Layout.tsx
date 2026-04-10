import React from "react";
import Navbar from "./Navbar";
import ToastContainer from "@/components/ui/ToastContainer";
import styles from "./Layout.module.css";

interface LayoutProps {
  children: React.ReactNode;
}

export default function Layout({ children }: LayoutProps) {
  return (
    <>
      <Navbar />
      <main className={styles.main}>{children}</main>
      <ToastContainer />
    </>
  );
}

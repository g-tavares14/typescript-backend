ALTER TABLE "transactions" ADD COLUMN "updated_at" timestamp with time zone DEFAULT now() NOT NULL;
-- O DEFAULT now() carimbaria os registros antigos com a data desta migration; eles nunca foram editados,
-- então a última alteração deles é a própria criação.
UPDATE "transactions" SET "updated_at" = "created_at";

ALTER TABLE public.booking ADD rating int2 NULL CHECK (rating >= 1);

/*
  HLab.Argyll
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Argyll.

    HLab.Argyll is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Argyll is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;

namespace HLab.Sys.Argyll;

public class Matrix2D
{
    //<attributs>
    private double[,] _matrice;
    private int _nbLignes;
    private int _nbColonnes;
    //</attributs>

    //<Constructeurs>
    //  Cas général:
    public Matrix2D(int n, int p)
    {
        this._matrice = new double[n, p];
        this._nbLignes = n;
        this._nbColonnes = p;
    }
    //  Copie d'une autre matrice:
    public Matrix2D(Matrix2D originale)
    {
        int n = originale._matrice.GetLength(0);
        int p = originale._matrice.GetLength(1);
        this._matrice = new double[n, p];
        this._nbColonnes = originale._nbColonnes;
        this._nbLignes = originale._nbLignes;
        Initialise(originale._matrice);
    }
    //  Cas initialisée par un tableau:
    public Matrix2D(double[,] tableau)
    {
        int n = tableau.GetLength(0);
        int p = tableau.GetLength(1);
        this._matrice = new double[n, p];
        this._nbLignes = n;
        this._nbColonnes = p;
        Initialise(tableau);
    }
    //  Matrix2D Carrée:
    public Matrix2D(int n)
    {
        this._matrice = new double[n, n];
        this._nbLignes = n;
        this._nbColonnes = n;
    }
    //</Constructeurs>

    //<indexeur>
    public double this[int n, int p]
    {
        get => _matrice[n, p]; set => _matrice[n, p] = value;
    }
    //</indexeur>

    //<Propriétés>
    public string Length
    {
        get
        {
            int n = _matrice.GetLength(0);
            int p = _matrice.GetLength(1);
            string length = "(" + n + "," + p + ")";
            return length;
        }
    }

    public Matrix2D Transpose
    {
        get
        {
            double[,] TableauTemporaire = new double[_nbColonnes, _nbLignes];

            for (int j = 0; j < _nbLignes; j++)
            {
                for (int i = 0; i < _nbColonnes; i++)
                {
                    TableauTemporaire[i, j] = _matrice[j, i];
                }
            }
            return new Matrix2D(TableauTemporaire);
        }

    }

    public double Determinant
    {
        get
        {
            double det = 0;
            Matrix2D B;

            //Conditions d'arrêt
            if (this._nbLignes == 1) return this[0, 0];
            if (this._nbLignes == 2) return (this[0, 0] * this[1, 1] - this[0, 1] * this[1, 0]);

            //Traitement par récursivité
            for (int j = 0; j < this._nbLignes; j++)
            {
                B = this.SousMatrice(0, j);
                if (j % 2 == 0) { det += this[0, j] * B.Determinant; }
                else { det += -1 * this[0, j] * B.Determinant; }
            }
            return det;
        }
    }

    public Matrix2D Comatrice
    {
        get
        {
            Matrix2D B = new Matrix2D(this._nbLignes, this._nbColonnes);
            Matrix2D S;

            for (int i = 0; i < B._nbColonnes; i++)
            {
                for (int j = 0; j < B._nbColonnes; j++)
                {
                    S = this.SousMatrice(i, j);
                    if ((i + j) % 2 == 0) { B[i, j] = S.Determinant; }
                    else { B[i, j] = -1 * S.Determinant; }
                }
            }

            return B;
        }
    }

    public Matrix2D Inverse
    {
        get
        {
            double det = this.Determinant;
            Matrix2D t_Comatrice = this.Comatrice.Transpose;
            Matrix2D Inverse;
            Inverse = t_Comatrice * (1 / det);
            return Inverse;
        }
    }

    public double Trace
    {
        get
        {
            double Trace = 0;
            try
            {
                if (this._nbLignes == this._nbColonnes)
                {
                    for (int i = 0; i < this._nbLignes; i++)
                    {
                        Trace += this[i, i];
                    }
                    return Trace;
                }
                else
                {
                    throw new Exception("Impossible de calculer la trace du matrice non carrée");
                }
            }
            catch (Exception e)
            {
                Console.Error.WriteLine("" + e);
                return Trace;
            }
        }
    }

    public bool IsCarree
    {
        get
        {
            if (this._nbLignes == this._nbColonnes) { return true; }
            else { return false; }
        }
    }

    public bool IsInversible
    {
        get
        {
            if (this.Determinant != 0) { return true; }
            else { return false; }
        }
    }
    //</Propriétés>

    //<Méthodes>
    public override string ToString()
    {
        string liste = "";
        for (int i = 0; i < _nbLignes; i++)
        {
            liste += "|";
            for (int j = 0; j < _nbColonnes; j++)
            {
                liste += "  " + _matrice[i, j];
            }
            liste += "  |\n";
        }
        return liste;
    }

    public void Initialise(double[,] tableau)
    {
        bool OK = false;
        for (int i = 0; i <= 1; i++)
        {
            if (this._matrice.GetLength(0) == tableau.GetLength(0))
            {
                OK = true;
            }
            else
            {
                break;
            }
        }
        try
        {
            if (OK)
            {
                _matrice = tableau;
            }
            else
            {
                throw new Exception("La dimension des données fournies ne correspond pas à la taille de la matrice.");
            }
        }
        catch (Exception e)
        {
            Console.Error.WriteLine("" + e);
        }
    }

    public void Initialise(string NomMatrice)
    {
        Console.WriteLine("---\nInitialisation de la matrice " + NomMatrice + " ( taille = " + this.Length + " )");

        for (int i = 0; i < this._nbLignes; i++)
        {
            for (int j = 0; j < this._nbColonnes; j++)
            {
                Console.Write(NomMatrice + "[" + (i + 1) + "," + (j + 1) + "]=");

                this[i, j] = Double.Parse(Console.ReadLine());
            }
        }
    }

    public Matrix2D SousMatrice(int ib, int jb)
    {
        Matrix2D B = new Matrix2D(this._nbLignes - 1, this._nbColonnes - 1);
        int ir = 0, jr = 0;
        for (int i = 0; i < this._nbLignes; i++)
        {
            for (int j = 0; j < this._nbColonnes; j++)
            {
                if (i != (ib) && j != (jb))
                {
                    B[ir, jr] = this[i, j];
                    if (jr < B._nbLignes - 1) jr++;
                    else { jr = 0; ir++; }
                }
            }
        }
        return B;
    }
    //</Méthodes>

    //<Opérateurs>
    public static Matrix2D operator +(Matrix2D A, Matrix2D B)
    {
        try
        {
            if (A.Length == B.Length)
            {
                Matrix2D C = new Matrix2D(A._nbLignes, A._nbColonnes);
                for (int i = 0; i < A._nbLignes; i++)
                {
                    for (int j = 0; j < A._nbColonnes; j++)
                    {
                        C[i, j] = A[i, j] + B[i, j];
                    }
                }
                return C;
            }
            else
            {
                throw new Exception("Impossible d'additionner des matrices de dimensions différentes");
            }
        }
        catch (Exception e)
        {
            Console.Error.WriteLine("" + e);
            Matrix2D C = new Matrix2D(1, 1);
            return C;
        }
    }

    public static Matrix2D operator -(Matrix2D A, Matrix2D B)
    {
        try
        {
            if (A.Length == B.Length)
            {
                Matrix2D C = new Matrix2D(A._nbLignes, A._nbColonnes);
                for (int i = 0; i < A._nbLignes; i++)
                {
                    for (int j = 0; j < A._nbColonnes; j++)
                    {
                        C[i, j] = A[i, j] - B[i, j];
                    }
                }
                return C;
            }
            else
            {
                throw new Exception("Impossible de soustraire des matrices de dimensions différentes");
            }
        }
        catch (Exception e)
        {
            Console.Error.WriteLine("" + e);
            Matrix2D C = new Matrix2D(1, 1);
            return C;
        }
    }

    public static Matrix2D operator *(Matrix2D A, Matrix2D B)
    {
        try
        {
            if (A._nbColonnes == B._nbLignes)
            {
                Matrix2D C = new Matrix2D(A._nbLignes, B._nbColonnes);
                for (int i = 0; i < A._nbLignes; i++)
                {
                    for (int j = 0; j < B._nbColonnes; j++)
                    {
                        for (int k = 0; k < A._nbColonnes; k++)
                        {
                            C[i, j] += A[i, k] * B[k, j];
                        }
                    }
                }
                return C;
            }
            else
            {
                throw new Exception("Impossible de multiplier les matrices, les dimensions ne correspondent pas");
            }
        }
        catch (Exception e)
        {
            Console.Error.WriteLine("" + e);
            Matrix2D C = new Matrix2D(1);
            return C;
        }
    }

    public static Matrix2D operator *(double n, Matrix2D A)
    {
        Matrix2D B = new Matrix2D(A);

        for (int i = 0; i < A._nbLignes; i++)
        {
            for (int j = 0; j < A._nbColonnes; j++)
            {
                B[i, j] = n * A[i, j];
            }
        }

        return B;
    }

    public static Matrix2D operator *(Matrix2D A, double n)
    {
        Matrix2D B;
        B = n * A;
        return B;
    }
    //</Opérateurs>
}